//! Native `.anim` container: a ZIP archive holding `document.json`
//! (format-versioned) and binary media under `media/<id>.<ext>`.

use crate::{Document, FORMAT_VERSION, bump_ids_above};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("invalid document JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("document format version {0} is newer than supported version {FORMAT_VERSION}")]
    TooNew(u32),
    #[error("missing media file {0}")]
    MissingMedia(String),
    #[error("{0}")]
    Other(String),
}

const MANIFEST: &str = "mimetype";
const MIME: &str = "application/x-animara";

pub fn save_to_bytes(doc: &Document) -> Result<Vec<u8>, DocumentError> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(MANIFEST, stored)?;
        zip.write_all(MIME.as_bytes())?;
        zip.start_file("document.json", deflated)?;
        let mut d = doc.clone();
        d.format_version = FORMAT_VERSION;
        zip.write_all(serde_json::to_string_pretty(&d)?.as_bytes())?;
        for b in &doc.library.bitmaps {
            zip.start_file(format!("media/{}.{}", b.id, b.format), stored)?;
            zip.write_all(&b.data)?;
        }
        for s in &doc.library.sounds {
            zip.start_file(format!("media/{}.{}", s.id, s.format), deflated)?;
            zip.write_all(&s.data)?;
        }
        zip.finish()?;
    }
    Ok(buf.into_inner())
}

pub fn load_from_bytes(bytes: &[u8]) -> Result<Document, DocumentError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut json = String::new();
    zip.by_name("document.json")?.read_to_string(&mut json)?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    let version = value.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version > FORMAT_VERSION {
        return Err(DocumentError::TooNew(version));
    }
    let value = migrate(value, version);
    let mut doc: Document = serde_json::from_value(value)?;
    let mut read_media = |name: String| -> Result<Arc<Vec<u8>>, DocumentError> {
        let mut f = zip.by_name(&name).map_err(|_| DocumentError::MissingMedia(name.clone()))?;
        let mut v = Vec::new();
        f.read_to_end(&mut v)?;
        Ok(Arc::new(v))
    };
    for b in &mut doc.library.bitmaps {
        b.data = read_media(format!("media/{}.{}", b.id, b.format))?;
    }
    for s in &mut doc.library.sounds {
        s.data = read_media(format!("media/{}.{}", s.id, s.format))?;
    }
    bump_ids_above(doc.max_id());
    Ok(doc)
}

/// Upgrade older JSON layouts to the current version.
fn migrate(value: serde_json::Value, _from: u32) -> serde_json::Value {
    // Version 1 is the first public layout; future migrations go here.
    value
}

pub fn save_document(doc: &Document, path: &Path) -> Result<(), DocumentError> {
    let bytes = save_to_bytes(doc)?;
    // Write atomically: temp file + rename, so a crash never truncates the project.
    let tmp = path.with_extension("anim.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_document(path: &Path) -> Result<Document, DocumentError> {
    let bytes = std::fs::read(path)?;
    let mut doc = load_from_bytes(&bytes)?;
    if let Some(stem) = path.file_stem() {
        doc.name = stem.to_string_lossy().into_owned();
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use animara_geometry::{Affine, Path as GPath, Rect, Vec2};

    fn sample() -> Document {
        let mut doc = Document::new(640.0, 480.0, 24.0);
        let mut sym = Symbol::new("Ball", SymbolKind::Graphic);
        sym.timeline.layers[0].frames[0].elements.push(Element::shape(vec![ShapePath::filled(
            GPath::ellipse(Rect::new(-20.0, -20.0, 20.0, 20.0)),
            Fill::Radial {
                stops: vec![
                    GradientStop { offset: 0.0, color: Color::WHITE },
                    GradientStop { offset: 1.0, color: Color::from_rgba8(200, 30, 30, 255) },
                ],
                matrix: Affine::scale(20.0, 20.0),
                spread: SpreadMode::Pad,
                focal: 0.0,
            },
        )]));
        let sid = doc.library.add_symbol(sym);
        let layer = &mut doc.scenes[0].timeline.layers[0];
        layer.frames[0].elements.push(doc_instance(sid, 100.0, 100.0));
        layer.extend_to(23);
        layer.insert_keyframe(23, None);
        layer.frames[1].elements[0].transform = Affine::translate(500.0, 300.0);
        layer.set_tween(0, Some(Tween::new(TweenKind::Classic)));
        layer.frames[0].label = Some(FrameLabel { name: "start".into(), kind: LabelKind::Name });
        layer.frames[0].script = "stop();".into();
        doc.library.bitmaps.push(BitmapAsset {
            id: fresh_id(),
            name: "pixel.png".into(),
            width: 1,
            height: 1,
            format: "png".into(),
            folder: "Images".into(),
            smoothing: true,
            data: Arc::new(vec![1, 2, 3, 4]),
        });
        let mut text = TextElement::new("Héllo مرحبا", TextStyle::default());
        text.rtl = true;
        doc.scenes[0].timeline.add_layer("Text", 0);
        doc.scenes[0].timeline.layers[0].frames[0].elements.push(Element::new(ElementKind::Text(text)));
        doc.scenes[0].timeline.layers[0].frames[0].elements[0].pivot = Vec2::new(3.0, 4.0);
        doc
    }

    fn doc_instance(sid: Id, x: f64, y: f64) -> Element {
        Element::new(ElementKind::Instance(SymbolInstance::new(sid))).with_transform(Affine::translate(x, y))
    }

    #[test]
    fn lossless_roundtrip() {
        let doc = sample();
        assert!(doc.validate().is_empty(), "{:?}", doc.validate());
        let bytes = save_to_bytes(&doc).unwrap();
        let back = load_from_bytes(&bytes).unwrap();
        assert_eq!(doc, back);
        // Saving again must produce identical document JSON.
        let again = load_from_bytes(&save_to_bytes(&back).unwrap()).unwrap();
        assert_eq!(back, again);
    }

    #[test]
    fn rejects_future_versions() {
        let mut doc = sample();
        doc.format_version = 99;
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            zip.start_file("document.json", zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(serde_json::to_string(&doc).unwrap().as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        assert!(matches!(load_from_bytes(&buf.into_inner()), Err(DocumentError::TooNew(99))));
    }

    #[test]
    fn undo_redo() {
        let mut doc = sample();
        let mut h = History::default();
        h.push("Rename", doc.clone());
        doc.name = "changed".into();
        assert_eq!(h.undo(&mut doc).as_deref(), Some("Rename"));
        assert_eq!(doc.name, "Untitled-1");
        h.redo(&mut doc);
        assert_eq!(doc.name, "changed");
    }
}
