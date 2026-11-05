//! Minimal XML DOM built on quick-xml (enough for XFL documents).

use quick_xml::events::Event;

#[derive(Debug, Clone, Default)]
pub struct XmlNode {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
    pub text: String,
}

impl XmlNode {
    pub fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    pub fn f(&self, k: &str) -> Option<f64> {
        self.attr(k).and_then(|v| v.trim().parse().ok())
    }
    pub fn f_or(&self, k: &str, d: f64) -> f64 {
        self.f(k).unwrap_or(d)
    }
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn kids<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlNode> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Children of a wrapper element: `node.list("layers", "DOMLayer")`.
    pub fn list<'a>(&'a self, wrapper: &str, item: &'a str) -> Vec<&'a XmlNode> {
        self.child(wrapper).map(|w| w.kids(item).collect()).unwrap_or_default()
    }
    pub fn all_text(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.all_text());
        }
        s
    }
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let ent = &rest[1..end];
        match ent {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                if let Some(c) = u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            e if e.starts_with('#') => {
                if let Some(c) = e[1..].parse::<u32>().ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            other => {
                out.push('&');
                out.push_str(other);
                out.push(';');
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn start_node(e: &quick_xml::events::BytesStart) -> XmlNode {
    let name = e.local_name().as_ref().to_string();
    let attrs = e.attributes().flatten().map(|a| (a.key.local_name().as_ref().to_string(), unescape(a.value.as_ref()))).collect();
    XmlNode { name, attrs, children: Vec::new(), text: String::new() }
}

pub fn parse(src: &str) -> Result<XmlNode, String> {
    let mut reader = quick_xml::Reader::from_str(src);
    let mut stack: Vec<XmlNode> = vec![XmlNode { name: "#document".into(), ..Default::default() }];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(start_node(&e)),
            Ok(Event::Empty(e)) => {
                let n = start_node(&e);
                stack.last_mut().unwrap().children.push(n);
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let n = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(n);
                }
            }
            Ok(Event::Text(t)) => {
                let raw: &str = t.as_ref();
                stack.last_mut().unwrap().text.push_str(&unescape(raw));
            }
            Ok(Event::CData(t)) => stack.last_mut().unwrap().text.push_str(t.as_ref()),
            Ok(Event::GeneralRef(r)) => {
                let name: &str = r.as_ref();
                let raw = format!("&{name};");
                stack.last_mut().unwrap().text.push_str(&unescape(&raw));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("{e} at byte {}", reader.buffer_position())),
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(n);
    }
    let doc = stack.pop().unwrap();
    doc.children.into_iter().next().ok_or_else(|| "empty XML document".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_with_entities() {
        let n = parse(r#"<?xml version="1.0"?><a x="1 &amp; 2"><b/><c>t&lt;x</c><d><![CDATA[stop();]]></d></a>"#).unwrap();
        assert_eq!(n.name, "a");
        assert_eq!(n.attr("x"), Some("1 & 2"));
        assert_eq!(n.child("c").unwrap().text, "t<x");
        assert_eq!(n.child("d").unwrap().text, "stop();");
    }
}
