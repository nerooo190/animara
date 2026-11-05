//! Per-document editing state: the document, history, edit context, selection and view.

use animara_animation::layer_elements_at;
use animara_document::{Document, Element, ElementKind, History, Id, Keyframe, Layer, LayerKind, Timeline};
use animara_geometry::{Affine, Rect, Vec2};
use animara_renderer::RenderCache;
use std::path::PathBuf;

/// One level of symbol editing (double-click into an instance / Edit Symbols).
#[derive(Debug, Clone)]
pub struct EditLevel {
    pub symbol: Id,
    /// Transform of the edited instance relative to the parent level (edit in place).
    pub instance_transform: Affine,
    pub in_place: bool,
    /// Playhead of the parent timeline when the level was entered.
    pub parent_frame: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSel {
    pub layer0: usize,
    pub layer1: usize,
    pub f0: u32,
    pub f1: u32,
}

impl FrameSel {
    pub fn single(layer: usize, f: u32) -> FrameSel {
        FrameSel { layer0: layer, layer1: layer, f0: f, f1: f }
    }
    pub fn layers(&self) -> std::ops::RangeInclusive<usize> {
        self.layer0.min(self.layer1)..=self.layer0.max(self.layer1)
    }
    pub fn frames(&self) -> (u32, u32) {
        (self.f0.min(self.f1), self.f0.max(self.f1))
    }
    pub fn contains(&self, layer: usize, f: u32) -> bool {
        let (a, b) = self.frames();
        self.layers().contains(&layer) && f >= a && f <= b
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OnionSkin {
    pub enabled: bool,
    pub before: u32,
    pub after: u32,
    pub outline: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct StageView {
    pub zoom: f64,
    /// Screen offset (points) of the document origin from the canvas rect's top-left.
    pub pan: Vec2,
    pub needs_fit: bool,
}

#[derive(Debug, Clone)]
pub struct TextEdit {
    pub element: Id,
    pub cursor: usize,
}

pub struct DocState {
    pub uid: u64,
    pub doc: Document,
    pub history: History,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub revision: u64,
    pub scene: usize,
    pub stack: Vec<EditLevel>,
    pub frame: u32,
    pub layer: usize,
    pub frame_sel: Option<FrameSel>,
    pub selection: Vec<Id>,
    pub playing: bool,
    pub play_anchor: Option<(f64, u32)>,
    pub loop_playback: bool,
    pub view: StageView,
    pub onion: OnionSkin,
    pub cache: RenderCache,
    pub texture: Option<egui::TextureHandle>,
    pub tex_key: u64,
    pub text_edit: Option<TextEdit>,
    /// Subselection: (element id, shape path index, command index).
    pub anchor_sel: Option<(Id, usize, usize)>,
    pub last_autosave_revision: u64,
}

static NEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DocState {
    pub fn new(doc: Document, path: Option<PathBuf>) -> DocState {
        DocState {
            uid: NEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            doc,
            history: History::default(),
            path,
            dirty: false,
            revision: 1,
            scene: 0,
            stack: Vec::new(),
            frame: 0,
            layer: 0,
            frame_sel: Some(FrameSel::single(0, 0)),
            selection: Vec::new(),
            playing: false,
            play_anchor: None,
            loop_playback: true,
            view: StageView { zoom: 1.0, pan: Vec2::ZERO, needs_fit: true },
            onion: OnionSkin { enabled: false, before: 2, after: 2, outline: false },
            cache: RenderCache::new(),
            texture: None,
            tex_key: 0,
            text_edit: None,
            anchor_sel: None,
            last_autosave_revision: 0,
        }
    }

    pub fn title(&self) -> String {
        format!("{}{}", self.doc.name, if self.dirty { "*" } else { "" })
    }

    pub fn current_symbol(&self) -> Option<Id> {
        self.stack.last().map(|l| l.symbol)
    }

    pub fn timeline(&self) -> &Timeline {
        let sym = self.current_symbol();
        match self.doc.timeline(self.scene, sym) {
            Some(t) => t,
            None => &self.doc.scenes[0].timeline,
        }
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        let sym = self.current_symbol();
        if self.doc.timeline(self.scene, sym).is_none() {
            self.stack.clear();
            self.scene = 0;
        }
        let sym = self.current_symbol();
        self.doc.timeline_mut(self.scene, sym).expect("valid timeline")
    }

    /// World transform of the current editing context (edit-in-place chain).
    pub fn context_transform(&self) -> Affine {
        let mut m = Affine::IDENTITY;
        for lvl in &self.stack {
            if lvl.in_place {
                m = m * lvl.instance_transform;
            } else {
                m = Affine::IDENTITY;
            }
        }
        m
    }

    /// Record an undo checkpoint before modifying the document.
    pub fn checkpoint(&mut self, label: &str) {
        self.history.push(label, self.doc.clone());
        self.dirty = true;
        self.revision += 1;
    }

    /// Mark a change that does not need its own undo step (e.g. live drag after a checkpoint).
    pub fn touch(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }

    pub fn undo(&mut self) -> Option<String> {
        let r = self.history.undo(&mut self.doc);
        self.after_history();
        r
    }

    pub fn redo(&mut self) -> Option<String> {
        let r = self.history.redo(&mut self.doc);
        self.after_history();
        r
    }

    fn after_history(&mut self) {
        self.revision += 1;
        self.dirty = true;
        self.clamp();
        let ids = self.visible_element_ids();
        self.selection.retain(|id| ids.contains(id));
        self.text_edit = None;
        self.anchor_sel = None;
    }

    /// Keep indices valid after structural edits.
    pub fn clamp(&mut self) {
        if self.scene >= self.doc.scenes.len() {
            self.scene = self.doc.scenes.len().saturating_sub(1);
        }
        while let Some(l) = self.stack.last() {
            if self.doc.library.symbol(l.symbol).is_none() {
                self.stack.pop();
            } else {
                break;
            }
        }
        let n = self.timeline().layers.len();
        if self.layer >= n {
            self.layer = n.saturating_sub(1);
        }
    }

    pub fn current_layer(&self) -> Option<&Layer> {
        self.timeline().layers.get(self.layer)
    }

    pub fn layer_editable(&self, li: usize) -> bool {
        let tl = self.timeline();
        li < tl.layers.len()
            && !tl.effectively_locked(li)
            && tl.effectively_visible(li)
            && !matches!(tl.layers[li].kind, LayerKind::Folder | LayerKind::Camera)
    }

    /// Keyframe of the current layer that owns the current frame, creating one when needed:
    /// past the layer end a keyframe is inserted; inside a tween span (when `auto_key`)
    /// a keyframe holding the interpolated state is inserted.
    pub fn editable_keyframe(&mut self, auto_key: bool) -> Option<&mut Keyframe> {
        let li = self.layer;
        if !self.layer_editable(li) {
            return None;
        }
        let f = self.frame;
        let (needs_insert, interpolated) = {
            let tl = self.timeline();
            let layer = &tl.layers[li];
            match layer.span_index(f) {
                None => (true, None),
                Some(i) => {
                    let k = &layer.frames[i];
                    if k.start != f && k.tween.is_some() && auto_key {
                        (true, Some(layer_elements_at(&self.doc, tl, li, f).0))
                    } else {
                        (false, None)
                    }
                }
            }
        };
        if needs_insert {
            let layer = &mut self.timeline_mut().layers[li];
            if f >= layer.end() {
                layer.insert_keyframe(f, None);
            } else {
                layer.insert_keyframe(f, interpolated);
            }
        }
        self.timeline_mut().layers[li].keyframe_at_mut(f)
    }

    /// Evaluated (tween-applied) elements of a layer at the current frame.
    pub fn layer_elements(&self, li: usize) -> Vec<Element> {
        let tl = self.timeline();
        if li >= tl.layers.len() {
            return Vec::new();
        }
        layer_elements_at(&self.doc, tl, li, self.frame).0
    }

    pub fn visible_element_ids(&self) -> Vec<Id> {
        (0..self.timeline().layers.len()).flat_map(|li| self.layer_elements(li).into_iter().map(|e| e.id)).collect()
    }

    /// Selected elements with their layer index (evaluated copies).
    pub fn selected(&self) -> Vec<(usize, Element)> {
        let mut out = Vec::new();
        for li in 0..self.timeline().layers.len() {
            for e in self.layer_elements(li) {
                if self.selection.contains(&e.id) {
                    out.push((li, e));
                }
            }
        }
        out
    }

    pub fn layer_of_element(&self, id: Id) -> Option<usize> {
        (0..self.timeline().layers.len()).find(|&li| self.layer_elements(li).iter().any(|e| e.id == id))
    }

    /// Mutable access to an element on the keyframe at the current frame, inserting an
    /// auto-keyframe when the frame is an interpolated tween frame.
    pub fn element_mut(&mut self, id: Id, auto_key: bool) -> Option<&mut Element> {
        let li = self.layer_of_element(id)?;
        let saved = self.layer;
        self.layer = li;
        let kf = self.editable_keyframe(auto_key);
        let found = kf.is_some();
        self.layer = saved;
        if !found {
            return None;
        }
        let f = self.frame;
        self.timeline_mut().layers[li].keyframe_at_mut(f)?.elements.iter_mut().find(|e| e.id == id)
    }

    /// Union of world bounds of the selection.
    pub fn selection_bounds(&self) -> Rect {
        let ctx = self.context_transform();
        self.selected()
            .iter()
            .fold(Rect::EMPTY, |r, (_, e)| r.union(&animara_renderer::bounds::element_bounds(&self.doc, e, &ctx)))
    }

    pub fn is_text(&self, id: Id) -> bool {
        self.selected().iter().any(|(_, e)| e.id == id && matches!(e.kind, ElementKind::Text(_)))
    }

    pub fn log_frame_bounds(&self) -> u32 {
        self.timeline().length()
    }
}
