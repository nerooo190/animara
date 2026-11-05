use crate::{Color, Ease, Element, Id, fresh_id};
use animara_geometry::{Path, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayerKind {
    #[default]
    Normal,
    /// Guide layers are visible while authoring but never exported. When other
    /// layers reference one via `guide_layer`, its first path is a motion guide.
    Guide,
    Mask,
    Folder,
    Camera,
    /// Armature (pose) layer created by the Bone tool.
    Armature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TweenKind {
    #[default]
    Classic,
    Shape,
    /// Property-based motion tween with an optional editable motion path.
    Motion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RotateMode {
    #[default]
    Auto,
    None,
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tween {
    pub kind: TweenKind,
    #[serde(default)]
    pub ease: Ease,
    #[serde(default)]
    pub rotate: RotateMode,
    /// Extra full turns for Clockwise / CounterClockwise.
    #[serde(default)]
    pub rotate_times: u32,
    #[serde(default)]
    pub orient_to_path: bool,
    #[serde(default = "yes")]
    pub scale: bool,
    #[serde(default = "yes")]
    pub snap: bool,
    /// Motion-tween path in parent coordinates, relative to the start position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_path: Option<Path>,
    /// Separate ease per property group (position, rotation, scale, color, filters).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub property_eases: Vec<(String, Ease)>,
}

fn yes() -> bool {
    true
}

impl Tween {
    pub fn new(kind: TweenKind) -> Tween {
        Tween {
            kind,
            ease: Ease::Linear,
            rotate: RotateMode::Auto,
            rotate_times: 0,
            orient_to_path: false,
            scale: true,
            snap: true,
            motion_path: None,
            property_eases: Vec::new(),
        }
    }
    pub fn ease_for(&self, property: &str) -> &Ease {
        self.property_eases.iter().find(|(p, _)| p == property).map(|(_, e)| e).unwrap_or(&self.ease)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LabelKind {
    #[default]
    Name,
    Comment,
    Anchor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameLabel {
    pub name: String,
    #[serde(default)]
    pub kind: LabelKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SoundSync {
    #[default]
    Event,
    Start,
    Stop,
    /// Stream sounds are locked to the timeline (scrubbing plays them).
    Stream,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoundRef {
    pub asset: Id,
    #[serde(default)]
    pub sync: SoundSync,
    #[serde(default)]
    pub loops: u32,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: f64,
    /// Fade in/out length in seconds.
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
}

fn one() -> f32 {
    1.0
}

impl SoundRef {
    pub fn new(asset: Id) -> SoundRef {
        SoundRef { asset, sync: SoundSync::Stream, loops: 0, volume: 1.0, trim_start: 0.0, trim_end: 0.0, fade_in: 0.0, fade_out: 0.0 }
    }
}

/// Camera-layer keyframe state. Position is the stage point at the view center.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<(Color, f32)>,
}

impl CameraState {
    pub fn centered(width: f64, height: f64) -> CameraState {
        CameraState { x: width / 2.0, y: height / 2.0, zoom: 1.0, rotation: 0.0, tint: None }
    }
    pub fn lerp(&self, o: &CameraState, t: f64) -> CameraState {
        use animara_geometry::lerp;
        CameraState {
            x: lerp(self.x, o.x, t),
            y: lerp(self.y, o.y, t),
            zoom: lerp(self.zoom, o.zoom, t),
            rotation: lerp(self.rotation, o.rotation, t),
            tint: self.tint,
        }
    }
}

/// Bone of an armature (inverse-kinematics rig).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub id: Id,
    pub name: String,
    pub parent: Option<usize>,
    /// Rest-pose head position: absolute for root bones, in parent-tip space for children.
    pub head: Vec2,
    pub length: f64,
    /// Rest angle relative to the parent bone, radians.
    pub angle: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_element: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_angle: Option<f64>,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Armature {
    pub bones: Vec<Bone>,
}

/// Pose of an armature: local angles per bone plus root head positions.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Pose {
    pub angles: Vec<f64>,
    #[serde(default)]
    pub root_offset: Vec2,
}

/// A keyframe and the run of frames it spans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub start: u32,
    pub duration: u32,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tween: Option<Tween>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<FrameLabel>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<Pose>,
}

impl Keyframe {
    pub fn new(start: u32, duration: u32) -> Keyframe {
        Keyframe {
            start,
            duration: duration.max(1),
            elements: Vec::new(),
            tween: None,
            label: None,
            script: String::new(),
            sound: None,
            camera: None,
            pose: None,
        }
    }
    pub fn end(&self) -> u32 {
        self.start + self.duration
    }
    pub fn contains(&self, f: u32) -> bool {
        f >= self.start && f < self.end()
    }
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
    /// Copy content into a new keyframe (fresh element ids), as F6 does.
    pub fn duplicate_content(&self, start: u32, duration: u32) -> Keyframe {
        Keyframe {
            start,
            duration: duration.max(1),
            elements: self.elements.iter().map(Element::duplicate_keep_id).collect(),
            tween: self.tween.clone(),
            label: None,
            script: String::new(),
            sound: None,
            camera: self.camera,
            pose: self.pose.clone(),
        }
    }
}

impl Element {
    /// Copy that preserves the element id. Tweens match elements across keyframes by id,
    /// so content copied into a new keyframe keeps its identity.
    pub fn duplicate_keep_id(&self) -> Element {
        self.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub kind: LayerKind,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub outline: bool,
    pub color: Color,
    /// Containing folder layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<Id>,
    /// Mask layer this layer is masked by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_layer: Option<Id>,
    /// Guide layer providing this layer's motion path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide_layer: Option<Id>,
    /// Layer parenting: this layer's content follows the parent layer's single element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_layer: Option<Id>,
    /// Folder expanded in the timeline.
    #[serde(default = "yes")]
    pub expanded: bool,
    /// Layer depth for camera parallax (0 = stage plane, positive = farther).
    #[serde(default)]
    pub depth: f64,
    #[serde(default)]
    pub frames: Vec<Keyframe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armature: Option<Armature>,
}

/// Default layer outline colors, cycled as layers are created.
pub const LAYER_COLORS: [Color; 8] = [
    Color::rgb(0.31, 0.89, 0.32),
    Color::rgb(1.0, 0.43, 0.06),
    Color::rgb(0.93, 0.23, 0.92),
    Color::rgb(0.29, 0.67, 1.0),
    Color::rgb(1.0, 0.85, 0.2),
    Color::rgb(0.0, 0.84, 0.84),
    Color::rgb(0.6, 0.4, 1.0),
    Color::rgb(1.0, 0.3, 0.3),
];

impl Layer {
    pub fn new(name: &str) -> Layer {
        let id = fresh_id();
        Layer {
            id,
            name: name.to_string(),
            kind: LayerKind::Normal,
            visible: true,
            locked: false,
            outline: false,
            color: LAYER_COLORS[(id.0 as usize) % LAYER_COLORS.len()],
            folder: None,
            mask_layer: None,
            guide_layer: None,
            parent_layer: None,
            expanded: true,
            depth: 0.0,
            frames: vec![Keyframe::new(0, 1)],
            armature: None,
        }
    }
    pub fn with_kind(mut self, kind: LayerKind) -> Layer {
        self.kind = kind;
        if kind == LayerKind::Folder {
            self.frames.clear();
        }
        self
    }

    /// Number of frames spanned by this layer.
    pub fn end(&self) -> u32 {
        self.frames.last().map(|k| k.end()).unwrap_or(0)
    }
    pub fn span_index(&self, f: u32) -> Option<usize> {
        let i = self.frames.partition_point(|k| k.end() <= f);
        (i < self.frames.len() && self.frames[i].contains(f)).then_some(i)
    }
    pub fn keyframe_at(&self, f: u32) -> Option<&Keyframe> {
        self.span_index(f).map(|i| &self.frames[i])
    }
    pub fn keyframe_at_mut(&mut self, f: u32) -> Option<&mut Keyframe> {
        self.span_index(f).map(move |i| &mut self.frames[i])
    }
    pub fn is_keyframe(&self, f: u32) -> bool {
        self.keyframe_at(f).is_some_and(|k| k.start == f)
    }
    pub fn next_keyframe_after(&self, f: u32) -> Option<u32> {
        self.frames.iter().map(|k| k.start).find(|&s| s > f)
    }
    pub fn prev_keyframe_before(&self, f: u32) -> Option<u32> {
        self.frames.iter().rev().map(|k| k.start).find(|&s| s < f)
    }

    fn fix_starts(&mut self) {
        let mut s = 0;
        for k in &mut self.frames {
            k.start = s;
            s += k.duration;
        }
    }

    /// Extend the layer so frame `f` exists (as an extension of the last keyframe).
    pub fn extend_to(&mut self, f: u32) {
        match self.frames.last_mut() {
            Some(last) => {
                if f >= last.end() {
                    last.duration = f + 1 - last.start;
                }
            }
            None => self.frames.push(Keyframe::new(0, f + 1)),
        }
    }

    /// Insert Frame (F5): add `n` frames at `f`, or extend to `f` past the end.
    pub fn insert_frames(&mut self, f: u32, n: u32) {
        if n == 0 {
            return;
        }
        if f >= self.end() {
            self.extend_to(f + n - 1);
            return;
        }
        if let Some(i) = self.span_index(f) {
            self.frames[i].duration += n;
            self.fix_starts();
        }
    }

    /// Remove Frames (Shift+F5).
    pub fn remove_frames(&mut self, f: u32, n: u32) {
        for _ in 0..n {
            let Some(i) = self.span_index(f) else { break };
            self.frames[i].duration -= 1;
            if self.frames[i].duration == 0 {
                self.frames.remove(i);
            }
            self.fix_starts();
        }
    }

    /// Ensure a keyframe boundary at `f`, copying content of the containing span.
    /// Returns the index of the keyframe starting at `f`.
    pub fn split_at(&mut self, f: u32) -> usize {
        if f >= self.end() {
            if f > self.end() {
                self.extend_to(f - 1);
            }
            let kf = match self.frames.last() {
                Some(last) => last.duration_content_or_blank(f),
                None => Keyframe::new(f, 1),
            };
            self.frames.push(kf);
            return self.frames.len() - 1;
        }
        let i = self.span_index(f).expect("frame inside layer");
        if self.frames[i].start == f {
            return i;
        }
        let old_end = self.frames[i].end();
        let new = self.frames[i].duplicate_content(f, old_end - f);
        self.frames[i].duration = f - self.frames[i].start;
        self.frames.insert(i + 1, new);
        i + 1
    }

    /// Insert Keyframe (F6). `content` overrides copied elements (e.g. interpolated tween state).
    pub fn insert_keyframe(&mut self, f: u32, content: Option<Vec<Element>>) -> bool {
        if self.is_keyframe(f) {
            return false;
        }
        let i = self.split_at(f);
        if let Some(c) = content {
            self.frames[i].elements = c;
        }
        true
    }

    /// Insert Blank Keyframe (F7).
    pub fn insert_blank_keyframe(&mut self, f: u32) -> bool {
        if self.is_keyframe(f) && !self.frames[self.span_index(f).unwrap()].is_empty() {
            // Animate converts the *next* frame when the current one is already a keyframe.
            return false;
        }
        let i = self.split_at(f);
        let k = &mut self.frames[i];
        k.elements.clear();
        k.tween = None;
        k.pose = None;
        true
    }

    /// Clear Keyframe (Shift+F6): merge the keyframe into the preceding span.
    pub fn clear_keyframe(&mut self, f: u32) -> bool {
        let Some(i) = self.span_index(f) else { return false };
        if self.frames[i].start != f {
            return false;
        }
        if i == 0 {
            let k = &mut self.frames[0];
            k.elements.clear();
            k.tween = None;
            return true;
        }
        let d = self.frames[i].duration;
        self.frames.remove(i);
        self.frames[i - 1].duration += d;
        true
    }

    /// Convert every frame in `range` to keyframes (Convert to Keyframes).
    pub fn convert_to_keyframes(&mut self, from: u32, to: u32) {
        for f in from..=to {
            if f < self.end() {
                self.split_at(f);
            }
        }
    }

    /// Drag a keyframe from `from` to `to` (must stay between its neighbours).
    pub fn move_keyframe(&mut self, from: u32, to: u32) -> bool {
        let Some(i) = self.span_index(from) else { return false };
        if self.frames[i].start != from || i == 0 || from == to {
            return false;
        }
        let prev_start = self.frames[i - 1].start;
        let next_start = self.frames.get(i + 1).map(|k| k.start);
        if to <= prev_start || next_start.is_some_and(|n| to >= n) {
            return false;
        }
        let end = self.frames[i].end();
        self.frames[i - 1].duration = to - prev_start;
        self.frames[i].start = to;
        self.frames[i].duration = end.saturating_sub(to).max(1);
        self.fix_starts();
        true
    }

    /// Copy frames `[from, to]` (inclusive) into a clipboard.
    pub fn copy_frames(&self, from: u32, to: u32) -> FrameClipboard {
        let mut tmp = self.clone();
        tmp.extend_to(to);
        tmp.split_at(from);
        if to + 1 < tmp.end() {
            tmp.split_at(to + 1);
        }
        let spans: Vec<Keyframe> = tmp
            .frames
            .into_iter()
            .filter(|k| k.start >= from && k.start <= to)
            .map(|mut k| {
                k.start -= from;
                k
            })
            .collect();
        FrameClipboard { spans, length: to - from + 1 }
    }

    /// Paste frames at `f`, overwriting the same number of existing frames.
    pub fn paste_frames(&mut self, f: u32, clip: &FrameClipboard) {
        if clip.length == 0 {
            return;
        }
        let end = f + clip.length;
        self.split_at(f);
        if end < self.end() {
            self.split_at(end);
        }
        self.frames.retain(|k| k.start < f || k.start >= end);
        let pos = self.frames.partition_point(|k| k.start < f);
        let inserted: Vec<Keyframe> = clip
            .spans
            .iter()
            .map(|k| {
                let mut k = k.clone();
                k.start += f;
                k.elements.iter_mut().for_each(Element::reassign_ids);
                k
            })
            .collect();
        self.frames.splice(pos..pos, inserted);
        self.fix_starts();
    }

    /// Reverse Frames for `[from, to]`.
    pub fn reverse_frames(&mut self, from: u32, to: u32) {
        if to <= from || from >= self.end() {
            return;
        }
        self.split_at(from);
        if to + 1 < self.end() {
            self.split_at(to + 1);
        }
        let a = self.frames.partition_point(|k| k.start < from);
        let b = self.frames.partition_point(|k| k.start <= to);
        self.frames[a..b].reverse();
        self.fix_starts();
    }

    pub fn set_tween(&mut self, f: u32, tween: Option<Tween>) -> bool {
        match self.keyframe_at_mut(f) {
            Some(k) => {
                k.tween = tween;
                true
            }
            None => false,
        }
    }

    /// Check structural invariants: contiguous spans starting at 0 with non-zero duration.
    pub fn validate(&self) -> Result<(), String> {
        let mut s = 0;
        for k in &self.frames {
            if k.start != s {
                return Err(format!("layer '{}': keyframe starts at {} but expected {}", self.name, k.start, s));
            }
            if k.duration == 0 {
                return Err(format!("layer '{}': zero-length keyframe at {}", self.name, k.start));
            }
            s += k.duration;
        }
        Ok(())
    }
}

impl Keyframe {
    fn duration_content_or_blank(&self, start: u32) -> Keyframe {
        self.duplicate_content(start, 1)
    }
}

/// Frames copied from one layer (Copy Frames / Paste Frames).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FrameClipboard {
    pub spans: Vec<Keyframe>,
    pub length: u32,
}

/// Ordered list of layers. Index 0 is the top-most layer, as in the timeline panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub layers: Vec<Layer>,
}

impl Timeline {
    pub fn with_layer(name: &str) -> Timeline {
        Timeline { layers: vec![Layer::new(name)] }
    }
    /// Total number of frames (longest layer).
    pub fn length(&self) -> u32 {
        self.layers.iter().map(Layer::end).max().unwrap_or(0).max(1)
    }
    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn layer_mut(&mut self, id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }
    pub fn layer_index(&self, id: Id) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }
    /// Insert a new layer above index `at`, padded to the timeline length.
    pub fn add_layer(&mut self, name: &str, at: usize) -> Id {
        let mut layer = Layer::new(name);
        let len = self.length();
        if len > 1 {
            layer.extend_to(len - 1);
        }
        let id = layer.id;
        self.layers.insert(at.min(self.layers.len()), layer);
        id
    }
    pub fn unique_layer_name(&self, base: &str) -> String {
        (1..)
            .map(|i| format!("{base} {i}"))
            .find(|n| !self.layers.iter().any(|l| &l.name == n))
            .expect("infinite iterator")
    }
    /// Remove a layer; folder children move to the folder's parent, references are cleared.
    pub fn remove_layer(&mut self, id: Id) -> Option<Layer> {
        let idx = self.layer_index(id)?;
        let removed = self.layers.remove(idx);
        for l in &mut self.layers {
            if l.folder == Some(id) {
                l.folder = removed.folder;
            }
            if l.mask_layer == Some(id) {
                l.mask_layer = None;
            }
            if l.guide_layer == Some(id) {
                l.guide_layer = None;
            }
            if l.parent_layer == Some(id) {
                l.parent_layer = None;
            }
        }
        Some(removed)
    }
    /// Frame labels (name labels) and their frame numbers.
    pub fn labels(&self) -> Vec<(u32, String)> {
        let mut out: Vec<(u32, String)> = self
            .layers
            .iter()
            .flat_map(|l| l.frames.iter())
            .filter_map(|k| k.label.as_ref().filter(|lb| lb.kind != LabelKind::Comment).map(|lb| (k.start, lb.name.clone())))
            .collect();
        out.sort();
        out
    }
    pub fn find_label(&self, name: &str) -> Option<u32> {
        self.labels().into_iter().find(|(_, n)| n == name).map(|(f, _)| f)
    }
    /// Is the layer (or any containing folder) hidden?
    pub fn effectively_visible(&self, idx: usize) -> bool {
        let mut cur = Some(&self.layers[idx]);
        let mut guard = 0;
        while let Some(l) = cur {
            if !l.visible {
                return false;
            }
            cur = l.folder.and_then(|f| self.layer(f));
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        true
    }
    pub fn effectively_locked(&self, idx: usize) -> bool {
        let mut cur = Some(&self.layers[idx]);
        let mut guard = 0;
        while let Some(l) = cur {
            if l.locked {
                return true;
            }
            cur = l.folder.and_then(|f| self.layer(f));
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        false
    }
    /// Nesting depth of a layer inside folders.
    pub fn folder_depth(&self, idx: usize) -> usize {
        let mut d = 0;
        let mut cur = self.layers[idx].folder;
        while let Some(f) = cur {
            d += 1;
            cur = self.layer(f).and_then(|l| l.folder);
            if d > 64 {
                break;
            }
        }
        d
    }
    /// Hidden in the panel because an ancestor folder is collapsed.
    pub fn is_collapsed_away(&self, idx: usize) -> bool {
        let mut cur = self.layers[idx].folder;
        let mut guard = 0;
        while let Some(f) = cur {
            match self.layer(f) {
                Some(l) if !l.expanded => return true,
                Some(l) => cur = l.folder,
                None => break,
            }
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        false
    }
    pub fn validate(&self) -> Result<(), String> {
        self.layers.iter().try_for_each(Layer::validate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, ShapePath};
    use animara_geometry::Rect;

    fn layer_with_content() -> Layer {
        let mut l = Layer::new("L");
        l.frames[0].elements.push(Element::shape(vec![ShapePath::filled(
            Path::rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
            crate::Fill::solid(Color::BLACK),
        )]));
        l
    }

    #[test]
    fn insert_frames_and_keyframes() {
        let mut l = layer_with_content();
        l.insert_frames(9, 1); // F5 at frame 10 extends to 10 frames
        assert_eq!(l.end(), 10);
        assert!(l.insert_keyframe(4, None));
        assert_eq!(l.frames.len(), 2);
        assert_eq!(l.frames[1].start, 4);
        assert_eq!(l.frames[1].duration, 6);
        assert_eq!(l.frames[1].elements.len(), 1);
        assert!(!l.insert_keyframe(4, None));
        assert!(l.insert_blank_keyframe(7));
        assert!(l.frames[2].elements.is_empty());
        l.validate().unwrap();
        l.insert_frames(1, 3);
        assert_eq!(l.end(), 13);
        assert_eq!(l.frames[1].start, 7);
        l.validate().unwrap();
    }

    #[test]
    fn remove_and_clear() {
        let mut l = layer_with_content();
        l.extend_to(9);
        l.insert_keyframe(5, None);
        l.remove_frames(0, 3);
        assert_eq!(l.end(), 7);
        assert_eq!(l.frames[1].start, 2);
        assert!(l.clear_keyframe(2));
        assert_eq!(l.frames.len(), 1);
        assert_eq!(l.frames[0].duration, 7);
        l.validate().unwrap();
    }

    #[test]
    fn keyframe_past_end_extends_previous() {
        let mut l = layer_with_content();
        l.insert_keyframe(20, None);
        assert_eq!(l.frames[0].duration, 20);
        assert_eq!(l.frames[1].start, 20);
        l.validate().unwrap();
    }

    #[test]
    fn copy_paste_reverse() {
        let mut l = layer_with_content();
        l.extend_to(9);
        l.insert_blank_keyframe(5);
        let clip = l.copy_frames(3, 6);
        assert_eq!(clip.length, 4);
        assert_eq!(clip.spans.len(), 2);
        l.paste_frames(10, &clip);
        assert_eq!(l.end(), 14);
        l.validate().unwrap();
        assert!(l.keyframe_at(11).unwrap().elements.len() == 1);
        assert!(l.keyframe_at(12).unwrap().elements.is_empty());
        l.reverse_frames(0, 9);
        l.validate().unwrap();
        assert!(l.keyframe_at(0).unwrap().elements.is_empty());
        assert!(!l.keyframe_at(9).unwrap().elements.is_empty());
    }

    #[test]
    fn move_keyframe_between_neighbours() {
        let mut l = layer_with_content();
        l.extend_to(9);
        l.insert_keyframe(5, None);
        assert!(l.move_keyframe(5, 7));
        assert_eq!(l.frames[0].duration, 7);
        assert_eq!(l.frames[1].start, 7);
        assert_eq!(l.end(), 10);
        assert!(!l.move_keyframe(7, 0));
        l.validate().unwrap();
    }

    #[test]
    fn timeline_layers() {
        let mut t = Timeline::with_layer("Layer 1");
        t.layers[0].extend_to(29);
        let id = t.add_layer("Layer 2", 0);
        assert_eq!(t.layers[0].id, id);
        assert_eq!(t.layers[0].end(), 30);
        assert_eq!(t.length(), 30);
        assert_eq!(t.unique_layer_name("Layer"), "Layer 3");
        t.remove_layer(id);
        assert_eq!(t.layers.len(), 1);
    }
}
