use crate::Document;

/// Snapshot-based undo/redo history. Documents share bulky media through `Arc`,
/// so snapshots are cheap relative to the reliability they provide.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<(String, Document)>,
    redo: Vec<(String, Document)>,
    pub limit: usize,
}

impl Default for History {
    fn default() -> Self {
        History { undo: Vec::new(), redo: Vec::new(), limit: 200 }
    }
}

impl History {
    /// Record the state *before* an edit.
    pub fn push(&mut self, label: &str, before: Document) {
        self.undo.push((label.to_string(), before));
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self, current: &mut Document) -> Option<String> {
        let (label, prev) = self.undo.pop()?;
        let now = std::mem::replace(current, prev);
        self.redo.push((label.clone(), now));
        Some(label)
    }
    pub fn redo(&mut self, current: &mut Document) -> Option<String> {
        let (label, next) = self.redo.pop()?;
        let now = std::mem::replace(current, next);
        self.undo.push((label.clone(), now));
        Some(label)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_labels(&self) -> impl Iterator<Item = &str> {
        self.undo.iter().map(|(l, _)| l.as_str())
    }
    pub fn redo_labels(&self) -> impl Iterator<Item = &str> {
        self.redo.iter().map(|(l, _)| l.as_str())
    }
    /// Jump to history position: `n` undo steps remain afterwards.
    pub fn goto(&mut self, n: usize, current: &mut Document) {
        while self.undo.len() > n && self.undo(current).is_some() {}
        while self.undo.len() < n && self.redo(current).is_some() {}
    }
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
