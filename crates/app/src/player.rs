//! Test Movie: an interactive player running frame scripts, movie clips,
//! buttons, events and audio against a private copy of the document.

use animara_animation::{EvalOptions, evaluate_document_frame, layer_elements_at};
use animara_document::{Document, Element, ElementKind, Id, SymbolKind};
use animara_geometry::{Affine, Vec2};
use animara_renderer::RenderCache;
use animara_scripting::{FrameRef, RuntimeInfo, ScriptCmd, ScriptHost, Target};
use std::collections::{HashMap, HashSet};

pub struct Player {
    pub doc: Document,
    pub scene: usize,
    pub frame: u32,
    pub playing: bool,
    host: ScriptHost,
    acc: f64,
    last_time: Option<f64>,
    clip_frames: HashMap<Id, u32>,
    clip_playing: HashMap<Id, bool>,
    clip_entered: HashSet<(Id, u32)>,
    hover: Option<Id>,
    pressed: Option<Id>,
    cache: RenderCache,
    texture: Option<egui::TextureHandle>,
    tex_key: (u32, usize, u64),
    revision: u64,
    pub log: Vec<String>,
    pub open: bool,
    audio: animara_media::AudioOut,
    pub mute: bool,
    started: bool,
}

impl Player {
    pub fn new(doc: Document, scene: usize, mute: bool) -> Player {
        Player {
            doc,
            scene,
            frame: 0,
            playing: true,
            host: ScriptHost::new(),
            acc: 0.0,
            last_time: None,
            clip_frames: HashMap::new(),
            clip_playing: HashMap::new(),
            clip_entered: HashSet::new(),
            hover: None,
            pressed: None,
            cache: RenderCache::new(),
            texture: None,
            tex_key: (u32::MAX, 0, 0),
            revision: 0,
            log: Vec::new(),
            open: true,
            audio: Default::default(),
            mute,
            started: false,
        }
    }

    fn timeline(&self) -> &animara_document::Timeline {
        &self.doc.scenes[self.scene].timeline
    }

    /// Top-level instances on the current root frame: (element, layer index).
    fn instances(&self) -> Vec<Element> {
        let tl = self.timeline();
        (0..tl.layers.len())
            .flat_map(|li| layer_elements_at(&self.doc, tl, li, self.frame).0)
            .filter(|e| matches!(e.kind, ElementKind::Instance(_)))
            .collect()
    }

    fn find_named(&self, name: &str) -> Option<Element> {
        if let Some(id) = name.strip_prefix('@') {
            return self.instances().into_iter().find(|e| e.id.0.to_string() == id);
        }
        let tl = self.timeline();
        (0..tl.layers.len()).flat_map(|li| layer_elements_at(&self.doc, tl, li, self.frame).0).find(|e| match &e.kind {
            ElementKind::Instance(i) => i.instance_name == name,
            ElementKind::Text(t) => t.instance_name == name,
            _ => false,
        })
    }

    fn resolve_frame(&self, tl: &animara_document::Timeline, f: &FrameRef) -> Option<u32> {
        match f {
            FrameRef::Number(n) => Some(n.saturating_sub(1).min(tl.length().saturating_sub(1))),
            FrameRef::Label(l) => tl.find_label(l),
        }
    }

    fn play_audio(&mut self) {
        if self.mute {
            return;
        }
        let tl = self.timeline().clone();
        let end = tl.length().saturating_sub(1).max(self.frame);
        let pcm = animara_media::mixdown_timeline(&self.doc, &tl, self.frame, end, 44100);
        if pcm.samples.iter().any(|s| *s != 0.0) {
            self.audio.play(&pcm);
        }
    }

    fn info(&self) -> RuntimeInfo {
        let mut positions = HashMap::new();
        for e in self.instances() {
            if let ElementKind::Instance(i) = &e.kind
                && !i.instance_name.is_empty()
            {
                positions.insert(i.instance_name.clone(), (e.transform.tx, e.transform.ty));
            }
        }
        RuntimeInfo { current_frame: self.frame, total_frames: self.timeline().length(), mouse: (0.0, 0.0), positions }
    }

    fn run_frame_scripts(&mut self) {
        let tl = self.timeline().clone();
        let mut scripts = Vec::new();
        for l in tl.layers.iter().rev() {
            if let Some(k) = l.keyframe_at(self.frame)
                && k.start == self.frame
                && !k.script.is_empty()
            {
                scripts.push(k.script.clone());
            }
        }
        self.host.set_info(self.info());
        for s in scripts {
            match self.host.run(&s, Target::Root) {
                Ok(cmds) => self.apply(cmds, None),
                Err(e) => self.log.push(format!("Script error (frame {}): {e}", self.frame + 1)),
            }
        }
        // Movie clip timelines.
        for e in self.instances() {
            let ElementKind::Instance(inst) = &e.kind else { continue };
            let Some(sym) = self.doc.library.symbol(inst.symbol).cloned() else { continue };
            if sym.kind != SymbolKind::MovieClip {
                continue;
            }
            let f = *self.clip_frames.entry(e.id).or_insert(0);
            if !self.clip_entered.insert((e.id, f)) {
                continue;
            }
            let name = if inst.instance_name.is_empty() { format!("@{}", e.id.0) } else { inst.instance_name.clone() };
            for l in &sym.timeline.layers {
                if let Some(k) = l.keyframe_at(f)
                    && k.start == f
                    && !k.script.is_empty()
                {
                    match self.host.run(&k.script, Target::Named(name.clone())) {
                        Ok(cmds) => self.apply(cmds, Some(e.id)),
                        Err(err) => self.log.push(format!("Script error ({} frame {}): {err}", sym.name, f + 1)),
                    }
                }
            }
        }
    }

    fn goto_root(&mut self, f: u32, play: bool) {
        self.frame = f;
        self.playing = play;
        self.clip_entered.clear();
        self.run_frame_scripts();
        if self.playing {
            self.play_audio();
        } else {
            self.audio.stop();
        }
    }

    fn apply(&mut self, cmds: Vec<ScriptCmd>, this_clip: Option<Id>) {
        for c in cmds {
            let target_clip = |p: &Player, t: &Target| -> Option<Option<Id>> {
                match t {
                    Target::Root => Some(None),
                    Target::This => Some(this_clip),
                    Target::Named(n) => p.find_named(n).map(|e| Some(e.id)).or_else(|| {
                        // `stop()` inside a clip resolves to that clip.
                        this_clip.filter(|_| n.starts_with('@')).map(Some)
                    }),
                }
            };
            match c {
                ScriptCmd::Trace(s) => self.log.push(s),
                ScriptCmd::Stop(ref t) | ScriptCmd::Play(ref t) => {
                    let play = matches!(c, ScriptCmd::Play(_));
                    let t = t.clone();
                    match target_clip(self, &t) {
                        Some(None) => {
                            self.playing = play;
                            if play { self.play_audio() } else { self.audio.stop() }
                        }
                        Some(Some(id)) => {
                            self.clip_playing.insert(id, play);
                        }
                        None => self.log.push(format!("Warning: no instance named {t:?}")),
                    }
                }
                ScriptCmd::Goto { target, frame, play } => match target_clip(self, &target) {
                    Some(None) => {
                        let tl = self.timeline().clone();
                        match self.resolve_frame(&tl, &frame) {
                            Some(f) => self.goto_root(f, play),
                            None => self.log.push(format!("Warning: frame {frame:?} not found")),
                        }
                    }
                    Some(Some(id)) => {
                        let sym = self
                            .instances()
                            .into_iter()
                            .find(|e| e.id == id)
                            .and_then(|e| e.as_instance().and_then(|i| self.doc.library.symbol(i.symbol).cloned()));
                        if let Some(s) = sym
                            && let Some(f) = self.resolve_frame(&s.timeline, &frame)
                        {
                            self.clip_frames.insert(id, f);
                            self.clip_playing.insert(id, play);
                        }
                    }
                    None => self.log.push(format!("Warning: no instance named {target:?}")),
                },
                ScriptCmd::NextFrame(ref t) | ScriptCmd::PrevFrame(ref t) => {
                    let next = matches!(c, ScriptCmd::NextFrame(_));
                    let t = t.clone();
                    match target_clip(self, &t) {
                        Some(None) => {
                            let len = self.timeline().length();
                            let f = if next { (self.frame + 1).min(len - 1) } else { self.frame.saturating_sub(1) };
                            self.goto_root(f, false);
                        }
                        Some(Some(id)) => {
                            let e = self.clip_frames.entry(id).or_insert(0);
                            *e = if next { *e + 1 } else { e.saturating_sub(1) };
                            self.clip_playing.insert(id, false);
                        }
                        None => {}
                    }
                }
                ScriptCmd::NextScene | ScriptCmd::PrevScene => {
                    let c = c.clone();
                    let n = self.doc.scenes.len();
                    self.scene = if matches!(c, ScriptCmd::NextScene) { (self.scene + 1).min(n - 1) } else { self.scene.saturating_sub(1) };
                    self.goto_root(0, true);
                }
                ScriptCmd::SetProp { target, prop, value } => {
                    if let Target::Named(n) = &target
                        && let Some(e) = self.find_named(n)
                    {
                        self.mutate_element(e.id, |el| match prop.as_str() {
                            "x" => el.transform.tx = value,
                            "y" => el.transform.ty = value,
                            "rotation" => {
                                let mut p = el.transform.decompose();
                                let d = value.to_radians() - p.skew_x;
                                p.skew_x += d;
                                p.skew_y += d;
                                el.transform = Affine::compose(&p);
                            }
                            "scale" | "scale_x" | "scale_y" => {
                                let mut p = el.transform.decompose();
                                if prop != "scale_y" {
                                    p.scale_x = value;
                                }
                                if prop != "scale_x" {
                                    p.scale_y = value;
                                }
                                el.transform = Affine::compose(&p);
                            }
                            "alpha" => {
                                if let Some(i) = el.as_instance_mut() {
                                    i.color_effect = animara_document::ColorEffect::Alpha { alpha: value as f32 };
                                }
                            }
                            "visible" => {
                                if let Some(i) = el.as_instance_mut() {
                                    i.visible = value != 0.0;
                                }
                            }
                            _ => {}
                        });
                    }
                }
                ScriptCmd::SetText { target, text } => {
                    if let Target::Named(n) = &target
                        && let Some(e) = self.find_named(n)
                    {
                        self.mutate_element(e.id, |el| {
                            if let ElementKind::Text(t) = &mut el.kind {
                                t.set_plain_text(&text);
                            }
                        });
                    }
                }
                ScriptCmd::StopAllSounds => self.audio.stop(),
                ScriptCmd::Navigate(url) => self.log.push(format!("navigateToURL: {url} (opening links is disabled in Test Movie)")),
            }
        }
        self.revision += 1;
    }

    fn mutate_element(&mut self, id: Id, f: impl FnOnce(&mut Element)) {
        let frame = self.frame;
        let scene = self.scene;
        for l in &mut self.doc.scenes[scene].timeline.layers {
            if let Some(k) = l.keyframe_at_mut(frame)
                && let Some(e) = k.elements.iter_mut().find(|e| e.id == id)
            {
                f(e);
                return;
            }
        }
    }

    /// Advance time; returns true when the picture changed.
    pub fn tick(&mut self, now: f64) -> bool {
        if !self.started {
            self.started = true;
            self.host.reset();
            self.run_frame_scripts();
            if self.playing {
                self.play_audio();
            }
            return true;
        }
        let dt = self.last_time.map(|t| now - t).unwrap_or(0.0).min(0.5);
        self.last_time = Some(now);
        self.acc += dt;
        let step = 1.0 / self.doc.fps.max(0.01);
        let mut changed = false;
        while self.acc >= step {
            self.acc -= step;
            changed = true;
            // Movie clips advance independently.
            for e in self.instances() {
                let Some(inst) = e.as_instance() else { continue };
                let Some(sym) = self.doc.library.symbol(inst.symbol) else { continue };
                if sym.kind != SymbolKind::MovieClip || !*self.clip_playing.entry(e.id).or_insert(true) {
                    continue;
                }
                let len = sym.timeline.length().max(1);
                let f = self.clip_frames.entry(e.id).or_insert(0);
                *f = (*f + 1) % len;
                let fv = *f;
                self.clip_entered.remove(&(e.id, fv));
            }
            if self.playing {
                let len = self.timeline().length();
                if self.frame + 1 >= len {
                    if self.scene + 1 < self.doc.scenes.len() {
                        self.scene += 1;
                        self.frame = 0;
                    } else if self.doc.publish.loop_playback {
                        self.scene = 0;
                        self.frame = 0;
                        self.play_audio();
                    } else {
                        self.playing = false;
                    }
                } else {
                    self.frame += 1;
                }
                self.clip_entered.clear();
            }
            self.run_frame_scripts();
            for t in self.host.handler_targets("enter_frame") {
                match self.host.fire("enter_frame", &t) {
                    Ok(c) => self.apply(c, None),
                    Err(e) => self.log.push(format!("enter_frame handler error: {e}")),
                }
            }
        }
        changed
    }

    fn eval_opts(&self) -> EvalOptions {
        let mut o = EvalOptions::export();
        o.instance_frames = self.clip_frames.clone();
        // Buttons show Up/Over/Down frames.
        for e in self.instances() {
            if let Some(i) = e.as_instance()
                && self.doc.library.symbol(i.symbol).is_some_and(|s| s.kind == SymbolKind::Button)
            {
                let f = if self.pressed == Some(e.id) {
                    2
                } else if self.hover == Some(e.id) {
                    1
                } else {
                    0
                };
                o.instance_frames.insert(e.id, f);
            }
        }
        o
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button(if self.playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                self.playing = !self.playing;
                if self.playing { self.play_audio() } else { self.audio.stop() }
            }
            if ui.button("⏮ Rewind").clicked() {
                self.scene = 0;
                self.host.reset();
                self.goto_root(0, true);
            }
            if ui.button("⏭ Step").clicked() {
                let len = self.timeline().length();
                let f = (self.frame + 1).min(len - 1);
                self.goto_root(f, false);
            }
            ui.label(format!("Scene {} · Frame {}/{}", self.scene + 1, self.frame + 1, self.timeline().length()));
        });
        let avail = ui.available_size();
        let s = (avail.x as f64 / self.doc.width).min(avail.y as f64 / self.doc.height).max(0.05);
        let ppp = ui.ctx().pixels_per_point() as f64;
        let size = egui::vec2((self.doc.width * s) as f32, (self.doc.height * s) as f32);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let key = (
            self.frame,
            self.scene,
            self.revision
                ^ ((s * ppp * 1000.0) as u64)
                ^ self.hover.map(|h| h.0).unwrap_or(0)
                ^ self.pressed.map(|p| p.0 << 1).unwrap_or(0)
                ^ self.clip_frames.values().map(|v| *v as u64).sum::<u64>(),
        );
        if key != self.tex_key || self.texture.is_none() {
            self.tex_key = key;
            let dl = evaluate_document_frame(&self.doc, self.scene, self.frame, &self.eval_opts());
            if let Some(pm) = animara_renderer::render_display_list(&dl, &self.doc, &mut self.cache, s * ppp, false) {
                let img = egui::ColorImage::from_rgba_premultiplied([pm.width() as usize, pm.height() as usize], pm.data());
                match &mut self.texture {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => self.texture = Some(ui.ctx().load_texture("test-movie", img, egui::TextureOptions::LINEAR)),
                }
            }
        }
        if let Some(t) = &self.texture {
            ui.painter().image(t.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        // Pointer interaction with buttons and named instances.
        let pointer = resp.hover_pos().map(|p| Vec2::new(((p.x - rect.min.x) as f64) / s, ((p.y - rect.min.y) as f64) / s));
        let dl_view = evaluate_document_frame(&self.doc, self.scene, self.frame, &EvalOptions::export()).view;
        let pointer = pointer.map(|p| dl_view.invert().map(|v| v.apply(p)).unwrap_or(p));
        let hit = pointer.and_then(|p| {
            self.instances().into_iter().rev().find(|e| animara_renderer::bounds::element_hit(&self.doc, e, &Affine::IDENTITY, p, 1.0))
        });
        let new_hover = hit.as_ref().map(|e| e.id);
        if new_hover != self.hover {
            if let Some(old) = self
                .hover
                .and_then(|h| self.instances().into_iter().find(|e| e.id == h))
                .and_then(|e| e.as_instance().map(|i| i.instance_name.clone()))
                && let Ok(c) = self.host.fire("mouse_out", &old)
            {
                self.apply(c, None);
            }
            if let Some(n) = hit.as_ref().and_then(|e| e.as_instance().map(|i| i.instance_name.clone()))
                && let Ok(c) = self.host.fire("mouse_over", &n)
            {
                self.apply(c, None);
            }
            self.hover = new_hover;
        }
        if let Some(e) = &hit {
            let is_button = e.as_instance().and_then(|i| self.doc.library.symbol(i.symbol)).is_some_and(|s| s.kind == SymbolKind::Button);
            let name = e.as_instance().map(|i| i.instance_name.clone()).unwrap_or_default();
            if is_button || self.host.handler_targets("click").contains(&name) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.is_pointer_button_down_on() {
                self.pressed = Some(e.id);
            }
            if resp.clicked() && !name.is_empty() {
                match self.host.fire("click", &name) {
                    Ok(c) => self.apply(c, None),
                    Err(err) => self.log.push(format!("click handler error: {err}")),
                }
            }
        }
        if !resp.is_pointer_button_down_on() {
            self.pressed = None;
        }
        // Keyboard events.
        let keys: Vec<egui::Key> = ui.input(|i| {
            i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, .. } = e { Some(*key) } else { None }).collect()
        });
        for k in keys {
            if let Ok(c) = self.host.fire("key", k.name()) {
                self.apply(c, None);
            }
        }
    }

    pub fn stop_audio(&mut self) {
        self.audio.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animara_document::{Keyframe, Layer};

    #[test]
    fn stop_and_goto_label() {
        let mut doc = Document::new(100.0, 100.0, 10.0);
        let l: &mut Layer = &mut doc.scenes[0].timeline.layers[0];
        l.frames = vec![Keyframe::new(0, 5), Keyframe::new(5, 5)];
        l.frames[0].script = "gotoAndStop(\"end\");".into();
        l.frames[1].label = Some(animara_document::FrameLabel { name: "end".into(), kind: animara_document::LabelKind::Name });
        l.frames[1].script = "trace(\"at end\");".into();
        let mut p = Player::new(doc, 0, true);
        p.tick(0.0);
        assert_eq!(p.frame, 5);
        assert!(!p.playing);
        assert_eq!(p.log, vec!["at end".to_string()]);
        p.tick(1.0);
        assert_eq!(p.frame, 5, "stopped playhead stays put");
    }
}
