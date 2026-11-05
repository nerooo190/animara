//! Timeline panel: layer list, frame grid, playhead, keyframe editing and playback controls.

use crate::commands::Command as C;
use crate::core::AppCore;
use crate::icons::{self, Icon};
use crate::ops;
use crate::state::FrameSel;
use animara_document::{LabelKind, LayerKind, TweenKind};
use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};

const ROW_H: f32 = 22.0;
const RULER_H: f32 = 20.0;
const LAYER_W: f32 = 230.0;

#[derive(Default, Clone)]
pub struct TimelineUiState {
    pub scroll: egui::Vec2,
    pub renaming: Option<(animara_document::Id, String)>,
    pub drag_layer: Option<usize>,
    pub drag_keyframe: Option<(usize, u32)>,
    pub drag_select: bool,
}

fn tween_color(k: TweenKind, dark: bool) -> Color32 {
    let base = match k {
        TweenKind::Classic => Color32::from_rgb(0x8e, 0x6b, 0xd6),
        TweenKind::Motion => Color32::from_rgb(0xe0, 0xa0, 0x3a),
        TweenKind::Shape => Color32::from_rgb(0x5d, 0xb8, 0x6a),
    };
    if dark { base.gamma_multiply(0.85) } else { base }
}

pub fn timeline_ui(core: &mut AppCore, ui: &mut egui::Ui, tls: &mut TimelineUiState) {
    if core.docs.is_empty() {
        ui.weak("No document");
        return;
    }
    header(core, ui);
    ui.separator();
    body(core, ui, tls);
}

fn header(core: &mut AppCore, ui: &mut egui::Ui) {
    let mut cmds: Vec<C> = Vec::new();
    let mut fw = core.settings.frame_width;
    let mut auto_key = core.settings.tools.auto_keyframe;
    let st = core.doc_mut().unwrap();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let sz = 22.0;
        if icons::button(ui, Icon::NewLayer, false, "New Layer", sz).clicked() {
            cmds.push(C::InsertLayer);
        }
        if icons::button(ui, Icon::NewFolder, false, "New Folder", sz).clicked() {
            cmds.push(C::InsertFolder);
        }
        if icons::button(ui, Icon::Delete, false, "Delete Layer", sz).clicked() {
            ops::delete_layer(st);
        }
        if icons::button(ui, Icon::Camera, st.timeline().layers.iter().any(|l| l.kind == LayerKind::Camera), "Add / Remove Camera", sz).clicked() {
            if let Some(ci) = st.timeline().layers.iter().position(|l| l.kind == LayerKind::Camera) {
                st.checkpoint("Remove Camera");
                let id = st.timeline().layers[ci].id;
                st.timeline_mut().remove_layer(id);
                st.clamp();
            } else {
                cmds.push(C::InsertCameraLayer);
            }
        }
        ui.separator();
        let mut fps = st.doc.fps;
        ui.label(egui::RichText::new("FPS").weak().small());
        if ui.add(egui::DragValue::new(&mut fps).range(0.01..=120.0).speed(0.1).max_decimals(2)).changed() {
            st.checkpoint("Frame Rate");
            st.doc.fps = fps;
        }
        let len = st.timeline().length();
        let mut f1 = st.frame + 1;
        if ui.add(egui::DragValue::new(&mut f1).range(1..=len.max(1) + 1000).suffix(" F")).changed() {
            st.frame = f1.saturating_sub(1);
            st.selection.clear();
        }
        ui.label(egui::RichText::new(format!("{:.2}s", st.frame as f64 / st.doc.fps)).weak().small());
        ui.separator();
        for (icon, tip, c) in [
            (Icon::Keyframe, "Insert Keyframe (F6)", C::InsertKeyframe),
            (Icon::BlankKeyframe, "Insert Blank Keyframe (F7)", C::InsertBlankKeyframe),
            (Icon::Frame, "Insert Frame (F5)", C::InsertFrame),
        ] {
            if icons::button(ui, icon, false, tip, sz).clicked() {
                cmds.push(c);
            }
        }
        if icons::button(ui, Icon::AutoKey, auto_key, "Auto Keyframe: editing a tweened frame inserts a keyframe", sz).clicked() {
            auto_key = !auto_key;
        }
        ui.separator();
        if icons::button(ui, Icon::Onion, st.onion.enabled, "Onion Skin (Alt+O)", sz).clicked() {
            st.onion.enabled = !st.onion.enabled;
        }
        if st.onion.enabled {
            ui.add(egui::DragValue::new(&mut st.onion.before).range(0..=20).prefix("◀"));
            ui.add(egui::DragValue::new(&mut st.onion.after).range(0..=20).prefix("▶"));
            ui.checkbox(&mut st.onion.outline, "Outlines");
        }
        ui.separator();
        if icons::button(ui, Icon::First, false, "Go to First Frame", sz).clicked() {
            cmds.push(C::Rewind);
        }
        if icons::button(ui, Icon::StepBack, false, "Step Back One Frame (,)", sz).clicked() {
            cmds.push(C::StepBackward);
        }
        if icons::button(ui, if st.playing { Icon::Stop } else { Icon::Play }, st.playing, "Play / Stop (Enter)", sz).clicked() {
            cmds.push(C::PlayStop);
        }
        if icons::button(ui, Icon::StepForward, false, "Step Forward One Frame (.)", sz).clicked() {
            cmds.push(C::StepForward);
        }
        if icons::button(ui, Icon::Last, false, "Go to Last Frame", sz).clicked() {
            cmds.push(C::GoToEnd);
        }
        if icons::button(ui, Icon::Loop, st.loop_playback, "Loop Playback", sz).clicked() {
            st.loop_playback = !st.loop_playback;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Slider::new(&mut fw, 4.0..=40.0).show_value(false)).on_hover_text("Frame width");
        });
    });
    core.settings.frame_width = fw;
    core.settings.tools.auto_keyframe = auto_key;
    for c in cmds {
        core.run(c);
    }
}

fn body(core: &mut AppCore, ui: &mut egui::Ui, tls: &mut TimelineUiState) {
    let dark = core.settings.dark;
    let fw = core.settings.frame_width.clamp(4.0, 40.0);
    let area = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(area, Sense::click_and_drag());
    let painter = ui.painter_at(area);
    let bg = if dark { Color32::from_gray(38) } else { Color32::from_gray(236) };
    let row_bg = if dark { Color32::from_gray(48) } else { Color32::from_gray(250) };
    let grid = if dark { Color32::from_gray(58) } else { Color32::from_gray(215) };
    let text = ui.visuals().text_color();
    painter.rect_filled(area, 0.0, bg);
    let frames_rect = Rect::from_min_max(pos2(area.min.x + LAYER_W, area.min.y + RULER_H), area.max);
    let layers_rect = Rect::from_min_max(pos2(area.min.x, area.min.y + RULER_H), pos2(area.min.x + LAYER_W, area.max.y));
    let ruler_rect = Rect::from_min_max(pos2(frames_rect.min.x, area.min.y), pos2(area.max.x, area.min.y + RULER_H));

    // Scrolling.
    if resp.hovered() {
        let (scroll, shift) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift));
        if shift {
            tls.scroll.x -= scroll.y + scroll.x;
        } else {
            tls.scroll.x -= scroll.x;
            tls.scroll.y -= scroll.y;
        }
    }
    let st = core.doc_mut().unwrap();
    let tl_len = st.timeline().length();
    let visible_rows: Vec<usize> = (0..st.timeline().layers.len()).filter(|&i| !st.timeline().is_collapsed_away(i)).collect();
    let content_h = visible_rows.len() as f32 * ROW_H;
    tls.scroll.y = tls.scroll.y.clamp(0.0, (content_h - frames_rect.height() + ROW_H).max(0.0));
    tls.scroll.x = tls.scroll.x.max(0.0);
    // Keep the playhead visible while playing.
    if st.playing {
        let px = st.frame as f32 * fw;
        if px < tls.scroll.x || px > tls.scroll.x + frames_rect.width() - fw * 2.0 {
            tls.scroll.x = (px - frames_rect.width() * 0.25).max(0.0);
        }
    }
    let first_frame = (tls.scroll.x / fw).floor() as u32;
    let n_frames = (frames_rect.width() / fw).ceil() as u32 + 2;
    let fx = |f: u32| frames_rect.min.x + f as f32 * fw - tls.scroll.x;
    let row_y = |r: usize| layers_rect.min.y + r as f32 * ROW_H - tls.scroll.y;

    // Ruler.
    painter.rect_filled(ruler_rect, 0.0, bg);
    for f in first_frame..first_frame + n_frames {
        let x = fx(f);
        if (f + 1) % 5 == 0 || f == 0 {
            painter.line_segment([pos2(x + fw / 2.0, ruler_rect.max.y - 6.0), pos2(x + fw / 2.0, ruler_rect.max.y)], Stroke::new(1.0, text.gamma_multiply(0.5)));
            if (f + 1) % 5 == 0 && fw * 5.0 > 24.0 || (f + 1) % 10 == 0 {
                painter.text(pos2(x + fw / 2.0, ruler_rect.min.y + 2.0), egui::Align2::CENTER_TOP, format!("{}", f + 1), egui::FontId::proportional(10.0), text.gamma_multiply(0.8));
            }
        }
        let fps = st.doc.fps.max(1.0);
        if (f as f64 / fps).fract() < 1e-9 && f > 0 {
            painter.text(pos2(x + 2.0, ruler_rect.min.y + 1.0), egui::Align2::LEFT_TOP, format!("{}s", (f as f64 / fps) as u32), egui::FontId::proportional(9.0), Color32::from_rgb(0x2b, 0x9c, 0xff));
        }
    }

    // Rows.
    let painter_frames = painter.with_clip_rect(frames_rect);
    let painter_layers = painter.with_clip_rect(layers_rect);
    let sel = st.frame_sel;
    let tl = st.timeline().clone();
    for (r, &li) in visible_rows.iter().enumerate() {
        let y = row_y(r);
        if y + ROW_H < layers_rect.min.y || y > layers_rect.max.y {
            continue;
        }
        let layer = &tl.layers[li];
        let row = Rect::from_min_max(pos2(frames_rect.min.x, y), pos2(frames_rect.max.x, y + ROW_H));
        let lrow = Rect::from_min_max(pos2(layers_rect.min.x, y), pos2(layers_rect.max.x, y + ROW_H));
        let active = li == st.layer;
        painter_layers.rect_filled(lrow.shrink2(vec2(0.0, 0.5)), 0.0, if active { ui.visuals().selection.bg_fill } else { row_bg });
        painter_frames.rect_filled(row.shrink2(vec2(0.0, 0.5)), 0.0, row_bg);
        // Frame cell grid.
        for f in first_frame..first_frame + n_frames {
            let x = fx(f);
            if (f + 1) % 5 == 0 {
                painter_frames.rect_filled(Rect::from_min_max(pos2(x, y), pos2(x + fw, y + ROW_H)), 0.0, grid.gamma_multiply(0.5));
            }
            painter_frames.line_segment([pos2(x + fw, y), pos2(x + fw, y + ROW_H)], Stroke::new(1.0, grid));
        }
        // Spans.
        for k in &layer.frames {
            if k.end() < first_frame || k.start > first_frame + n_frames {
                continue;
            }
            let span = Rect::from_min_max(pos2(fx(k.start), y + 1.0), pos2(fx(k.end()), y + ROW_H - 1.0));
            let fill = match &k.tween {
                Some(t) => tween_color(t.kind, dark),
                None if k.elements.is_empty() && k.sound.is_none() && k.camera.is_none() => if dark { Color32::from_gray(66) } else { Color32::WHITE },
                None => if dark { Color32::from_gray(92) } else { Color32::from_gray(205) },
            };
            painter_frames.rect_filled(span, 0.0, fill);
            painter_frames.rect_stroke(span, 0.0, Stroke::new(1.0, Color32::from_gray(20)), egui::StrokeKind::Inside);
            let kx = fx(k.start) + fw / 2.0;
            let ky = y + ROW_H - 6.0;
            let has_content = !k.elements.is_empty() || k.camera.is_some() || k.pose.is_some();
            if has_content {
                painter_frames.circle_filled(pos2(kx, ky), 3.0, Color32::BLACK);
            } else {
                painter_frames.circle(pos2(kx, ky), 3.0, Color32::WHITE, Stroke::new(1.0, Color32::BLACK));
            }
            if k.duration > 1 {
                let ex = fx(k.end() - 1);
                if k.tween.is_some() {
                    painter_frames.arrow(pos2(kx + 5.0, y + ROW_H / 2.0), vec2(ex - kx - 2.0, 0.0), Stroke::new(1.0, Color32::from_gray(20)));
                } else {
                    painter_frames.rect_stroke(Rect::from_min_max(pos2(ex + 2.0, y + 5.0), pos2(ex + fw - 2.0, y + ROW_H - 4.0)), 0.0, Stroke::new(1.0, Color32::from_gray(30)), egui::StrokeKind::Middle);
                }
            }
            if let Some(lb) = &k.label {
                let (icon, col) = match lb.kind {
                    LabelKind::Name => ("⚑", Color32::from_rgb(0xd0, 0x30, 0x30)),
                    LabelKind::Comment => ("//", Color32::from_rgb(0x30, 0x90, 0x30)),
                    LabelKind::Anchor => ("⚓", Color32::from_rgb(0xd0, 0x80, 0x10)),
                };
                painter_frames.text(pos2(kx - 3.0, y + 1.0), egui::Align2::LEFT_TOP, format!("{icon} {}", lb.name), egui::FontId::proportional(10.0), col);
            }
            if !k.script.is_empty() {
                painter_frames.text(pos2(kx - 2.0, y + 1.0), egui::Align2::LEFT_TOP, "a", egui::FontId::proportional(10.0), Color32::from_gray(20));
            }
            if let Some(snd) = &k.sound
                && let Some(peaks) = core.audio.waveform(&st.doc, snd.asset)
            {
                // Draw the waveform across the span (frame-locked).
                let fps = st.doc.fps.max(1.0);
                let mut pts_top = Vec::new();
                let mid = y + ROW_H / 2.0;
                for f in k.start.max(first_frame)..k.end().min(first_frame + n_frames) {
                    let tsec = (f - k.start) as f64 / fps + snd.trim_start;
                    let idx = (tsec * 100.0) as usize;
                    let a = peaks.get(idx).copied().unwrap_or(0.0) * snd.volume;
                    let x = fx(f) + fw / 2.0;
                    pts_top.push((x, a));
                }
                for (x, a) in pts_top {
                    let h = a * (ROW_H / 2.0 - 2.0);
                    painter_frames.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(fw.min(3.0), Color32::from_rgb(0x2b, 0x6c, 0xd0)));
                }
            }
        }
        // Frame selection highlight.
        if let Some(s) = sel
            && s.layers().contains(&li)
        {
            let (a, b) = s.frames();
            let rr = Rect::from_min_max(pos2(fx(a), y), pos2(fx(b + 1), y + ROW_H));
            painter_frames.rect_filled(rr, 0.0, Color32::from_rgba_unmultiplied(0x2b, 0x9c, 0xff, 70));
        }
        // Layer column.
        let depth = tl.folder_depth(li) as f32;
        let mut x = lrow.min.x + 4.0 + depth * 12.0;
        let icon = match layer.kind {
            LayerKind::Folder => Icon::Folder,
            LayerKind::Mask => Icon::Mask,
            LayerKind::Guide => Icon::Guide,
            LayerKind::Camera => Icon::Camera,
            LayerKind::Armature => Icon::Armature,
            LayerKind::Normal if layer.mask_layer.is_some() => Icon::Masked,
            LayerKind::Normal => Icon::Layer,
        };
        if layer.kind == LayerKind::Folder {
            painter_layers.text(pos2(x, y + ROW_H / 2.0), egui::Align2::LEFT_CENTER, if layer.expanded { "▾" } else { "▸" }, egui::FontId::proportional(12.0), text);
            x += 10.0;
        }
        icons::paint(&painter_layers, Rect::from_min_size(pos2(x, y + 3.0), vec2(16.0, 16.0)), icon, text);
        x += 20.0;
        let name_rect = Rect::from_min_max(pos2(x, y), pos2(lrow.max.x - 70.0, y + ROW_H));
        if tls.renaming.as_ref().is_none_or(|(id, _)| *id != layer.id) {
            painter_layers.text(pos2(x, y + ROW_H / 2.0), egui::Align2::LEFT_CENTER, &layer.name, egui::FontId::proportional(12.5), text);
            if layer.parent_layer.is_some() {
                icons::paint(&painter_layers, Rect::from_min_size(pos2(name_rect.max.x - 14.0, y + 5.0), vec2(12.0, 12.0)), Icon::Parent, text.gamma_multiply(0.6));
            }
        }
        // Toggles: color/outline, eye, lock.
        let cx = lrow.max.x - 62.0;
        let col_rect = Rect::from_center_size(pos2(cx, y + ROW_H / 2.0), vec2(9.0, 11.0));
        let lc = crate::stage::c32(layer.color);
        if layer.outline {
            painter_layers.rect_stroke(col_rect, 0.0, Stroke::new(1.5, lc), egui::StrokeKind::Middle);
        } else {
            painter_layers.rect_filled(col_rect, 0.0, lc);
        }
        icons::paint(&painter_layers, Rect::from_center_size(pos2(lrow.max.x - 40.0, y + ROW_H / 2.0), vec2(16.0, 16.0)), if layer.visible { Icon::Eye } else { Icon::EyeOff }, if layer.visible { text.gamma_multiply(0.7) } else { Color32::from_rgb(0xd0, 0x40, 0x40) });
        icons::paint(&painter_layers, Rect::from_center_size(pos2(lrow.max.x - 16.0, y + ROW_H / 2.0), vec2(16.0, 16.0)), if layer.locked { Icon::Lock } else { Icon::Unlock }, if layer.locked { text } else { text.gamma_multiply(0.3) });
    }
    // Playhead.
    let ph = fx(st.frame) + fw / 2.0;
    if ph >= frames_rect.min.x && ph <= frames_rect.max.x {
        let col = Color32::from_rgb(0x2b, 0x80, 0xff);
        painter.rect_filled(Rect::from_center_size(pos2(ph, ruler_rect.center().y), vec2(fw.max(8.0), RULER_H - 4.0)), 2.0, col);
        painter.text(pos2(ph, ruler_rect.center().y), egui::Align2::CENTER_CENTER, format!("{}", st.frame + 1), egui::FontId::proportional(9.5), Color32::WHITE);
        painter.line_segment([pos2(ph, frames_rect.min.y), pos2(ph, frames_rect.max.y)], Stroke::new(1.0, col));
    }
    // Header strip for layer column.
    painter.rect_filled(Rect::from_min_max(area.min, pos2(area.min.x + LAYER_W, area.min.y + RULER_H)), 0.0, bg);
    painter.text(pos2(area.min.x + 8.0, area.min.y + RULER_H / 2.0), egui::Align2::LEFT_CENTER, format!("{} frames", tl_len), egui::FontId::proportional(10.0), text.gamma_multiply(0.6));

    // ---------------- Interaction
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let row_at = |p: Pos2| -> Option<usize> {
        if p.y < layers_rect.min.y {
            return None;
        }
        let r = ((p.y - layers_rect.min.y + tls.scroll.y) / ROW_H) as usize;
        visible_rows.get(r).copied()
    };
    let frame_at = |p: Pos2| -> u32 { ((p.x - frames_rect.min.x + tls.scroll.x) / fw).max(0.0) as u32 };
    let shift = ui.input(|i| i.modifiers.shift);
    let mut ctx_layer: Option<usize> = None;
    if let Some(p) = pointer {
        // Ruler scrub.
        if (resp.clicked() || resp.dragged()) && ruler_rect.contains(ui.input(|i| i.pointer.press_origin()).unwrap_or(p)) {
            st.frame = frame_at(p).min(tl_len + 500);
            st.playing = false;
            st.selection.clear();
        } else if layers_rect.contains(ui.input(|i| i.pointer.press_origin()).unwrap_or(p)) {
            if let Some(li) = row_at(ui.input(|i| i.pointer.press_origin()).unwrap_or(p)) {
                let y = row_y(visible_rows.iter().position(|&x| x == li).unwrap_or(0));
                let lrow_max = layers_rect.max.x;
                if resp.clicked() {
                    let px = p.x;
                    if (px - (lrow_max - 16.0)).abs() < 9.0 {
                        st.checkpoint("Lock Layer");
                        let l = &mut st.timeline_mut().layers[li];
                        l.locked = !l.locked;
                    } else if (px - (lrow_max - 40.0)).abs() < 9.0 {
                        st.checkpoint("Show/Hide Layer");
                        let l = &mut st.timeline_mut().layers[li];
                        l.visible = !l.visible;
                    } else if (px - (lrow_max - 62.0)).abs() < 7.0 {
                        st.checkpoint("Outline Layer");
                        let l = &mut st.timeline_mut().layers[li];
                        l.outline = !l.outline;
                    } else if tl.layers[li].kind == LayerKind::Folder && px < layers_rect.min.x + 18.0 + tl.folder_depth(li) as f32 * 12.0 {
                        let l = &mut st.timeline_mut().layers[li];
                        l.expanded = !l.expanded;
                        st.revision += 1;
                    } else {
                        st.layer = li;
                        st.frame_sel = Some(FrameSel { layer0: li, layer1: li, f0: 0, f1: tl.layers[li].end().saturating_sub(1) });
                        // Select everything on that layer at the current frame (like clicking a layer name).
                        st.selection = st.layer_elements(li).into_iter().map(|e| e.id).collect();
                    }
                }
                if resp.double_clicked() && p.x < lrow_max - 70.0 {
                    tls.renaming = Some((tl.layers[li].id, tl.layers[li].name.clone()));
                }
                if resp.drag_started() {
                    tls.drag_layer = Some(li);
                }
                let _ = y;
            }
        } else if frames_rect.contains(ui.input(|i| i.pointer.press_origin()).unwrap_or(p)) {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
            if let Some(li) = row_at(origin) {
                let f0 = frame_at(origin);
                if resp.drag_started() {
                    let is_key = tl.layers[li].is_keyframe(f0);
                    let already = st.frame_sel.is_some_and(|s| s.contains(li, f0) && s.f0 == s.f1);
                    if is_key && already && f0 > 0 {
                        tls.drag_keyframe = Some((li, f0));
                    } else {
                        tls.drag_select = true;
                        st.frame_sel = Some(FrameSel::single(li, f0));
                        st.layer = li;
                        st.frame = f0;
                    }
                }
                if resp.clicked() {
                    st.layer = li;
                    if shift && let Some(s) = st.frame_sel {
                        st.frame_sel = Some(FrameSel { layer1: li, f1: f0, ..s });
                    } else {
                        st.frame_sel = Some(FrameSel::single(li, f0));
                        st.frame = f0;
                        st.playing = false;
                    }
                    st.selection = st.layer_elements(li).into_iter().map(|e| e.id).collect();
                    if !st.layer_editable(li) {
                        st.selection.clear();
                    }
                }
                if resp.double_clicked()
                    && let Some(k) = tl.layers[li].keyframe_at(f0)
                {
                    st.frame_sel = Some(FrameSel { layer0: li, layer1: li, f0: k.start, f1: k.end() - 1 });
                }
            }
            if resp.dragged() && tls.drag_select {
                if let (Some(li), Some(s)) = (row_at(p), st.frame_sel) {
                    let f = frame_at(p);
                    st.frame_sel = Some(FrameSel { layer1: li, f1: f, ..s });
                    st.frame = f.min(tl_len.saturating_sub(1).max(f));
                }
            }
        }
        if resp.secondary_clicked() {
            if frames_rect.contains(p)
                && let Some(li) = row_at(p)
            {
                let f = frame_at(p);
                if !st.frame_sel.is_some_and(|s| s.contains(li, f)) {
                    st.frame_sel = Some(FrameSel::single(li, f));
                    st.frame = f;
                }
                st.layer = li;
            } else if layers_rect.contains(p)
                && let Some(li) = row_at(p)
            {
                st.layer = li;
                ctx_layer = Some(li);
            }
        }
        if resp.drag_stopped() {
            if let Some((li, from)) = tls.drag_keyframe.take() {
                let to = frame_at(p);
                if to != from {
                    st.checkpoint("Move Keyframe");
                    if st.timeline_mut().layers[li].move_keyframe(from, to) {
                        st.frame_sel = Some(FrameSel::single(li, to));
                        st.frame = to;
                    } else {
                        st.undo();
                    }
                }
            }
            if let Some(from) = tls.drag_layer.take()
                && let Some(to) = row_at(p).or(Some(tl.layers.len()))
                && to != from
            {
                ops::move_layer(st, from, if to > from { to + 1 } else { to });
            }
            tls.drag_select = false;
        }
        if let Some(from) = tls.drag_layer
            && resp.dragged()
        {
            let r = visible_rows.iter().position(|&x| x == row_at(p).unwrap_or(from)).unwrap_or(0);
            let y = row_y(r);
            painter.line_segment([pos2(layers_rect.min.x, y), pos2(layers_rect.max.x, y)], Stroke::new(2.0, Color32::from_rgb(0x2b, 0x9c, 0xff)));
        }
        if let Some((li, _)) = tls.drag_keyframe
            && resp.dragged()
        {
            let r = visible_rows.iter().position(|&x| x == li).unwrap_or(0);
            let x = fx(frame_at(p));
            painter.rect_stroke(Rect::from_min_size(pos2(x, row_y(r)), vec2(fw, ROW_H)), 0.0, Stroke::new(2.0, Color32::from_rgb(0x2b, 0x9c, 0xff)), egui::StrokeKind::Middle);
        }
    }
    // Inline rename.
    if let Some((id, mut name)) = tls.renaming.clone() {
        if let Some(li) = tl.layer_index(id)
            && let Some(r) = visible_rows.iter().position(|&x| x == li)
        {
            let y = row_y(r);
            let rect = Rect::from_min_max(pos2(layers_rect.min.x + 26.0 + tl.folder_depth(li) as f32 * 12.0, y + 1.0), pos2(layers_rect.max.x - 72.0, y + ROW_H - 1.0));
            let te = ui.put(rect, egui::TextEdit::singleline(&mut name).id_salt("layer_rename"));
            te.request_focus();
            if te.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !name.trim().is_empty() {
                    st.checkpoint("Rename Layer");
                    if let Some(l) = st.timeline_mut().layer_mut(id) {
                        l.name = name.trim().to_string();
                    }
                }
                tls.renaming = None;
            } else {
                tls.renaming = Some((id, name));
            }
        } else {
            tls.renaming = None;
        }
    }
    let _ = ctx_layer;
    // Context menus.
    let press = ui.input(|i| i.pointer.press_origin()).or(pointer);
    let in_layers = press.is_some_and(|p| layers_rect.contains(p));
    let mut cmds: Vec<C> = Vec::new();
    resp.context_menu(|ui| {
        if in_layers {
            layer_menu(st, ui, &mut cmds);
        } else {
            for c in [
                C::CreateMotionTween,
                C::CreateShapeTween,
                C::CreateClassicTween,
                C::RemoveTween,
                C::InsertFrame,
                C::RemoveFrames,
                C::InsertKeyframe,
                C::InsertBlankKeyframe,
                C::ClearKeyframe,
                C::ConvertToKeyframes,
                C::ReverseFrames,
            ] {
                if ui.button(c.label()).clicked() {
                    cmds.push(c);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Copy Frames").clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("copy_frames"), true));
                ui.close();
            }
            if ui.button("Cut Frames").clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("cut_frames"), true));
                ui.close();
            }
            if ui.button("Paste Frames").clicked() {
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("paste_frames"), true));
                ui.close();
            }
            ui.separator();
            if ui.button("Select All Frames").clicked() {
                let n = st.timeline().layers.len();
                st.frame_sel = Some(FrameSel { layer0: 0, layer1: n.saturating_sub(1), f0: 0, f1: tl_len.saturating_sub(1) });
                ui.close();
            }
        }
    });
    let flags = ui.ctx().data_mut(|d| {
        let c = d.remove_temp::<bool>(egui::Id::new("copy_frames")).is_some();
        let x = d.remove_temp::<bool>(egui::Id::new("cut_frames")).is_some();
        let p = d.remove_temp::<bool>(egui::Id::new("paste_frames")).is_some();
        (c, x, p)
    });
    if flags.0 || flags.1 {
        ops::copy_frames(st, &mut core.clipboard);
        if flags.1 {
            ops::remove_frames(st);
        }
    }
    if flags.2 {
        let clip = core.clipboard.clone();
        ops::paste_frames(core.doc_mut().unwrap(), &clip);
    }
    for c in cmds {
        core.run(c);
    }
}

fn layer_menu(st: &mut crate::state::DocState, ui: &mut egui::Ui, cmds: &mut Vec<C>) {
    let li = st.layer;
    let Some(layer) = st.timeline().layers.get(li).cloned() else { return };
    if ui.button("Show All").clicked() {
        st.checkpoint("Show All");
        st.timeline_mut().layers.iter_mut().for_each(|l| l.visible = true);
        ui.close();
    }
    if ui.button("Lock Others").clicked() {
        st.checkpoint("Lock Others");
        for (i, l) in st.timeline_mut().layers.iter_mut().enumerate() {
            l.locked = i != li;
        }
        ui.close();
    }
    if ui.button("Hide Others").clicked() {
        st.checkpoint("Hide Others");
        for (i, l) in st.timeline_mut().layers.iter_mut().enumerate() {
            l.visible = i == li;
        }
        ui.close();
    }
    ui.separator();
    if ui.button("Insert Layer").clicked() {
        cmds.push(C::InsertLayer);
        ui.close();
    }
    if ui.button("Delete Layer").clicked() {
        ops::delete_layer(st);
        ui.close();
    }
    if ui.button("Duplicate Layer").clicked() {
        ops::duplicate_layer(st);
        ui.close();
    }
    ui.separator();
    let mut set_kind = |st: &mut crate::state::DocState, kind: LayerKind, label: &str| {
        st.checkpoint(label);
        let l = &mut st.timeline_mut().layers[li];
        l.kind = kind;
        if kind != LayerKind::Normal {
            l.mask_layer = None;
        }
    };
    if ui.selectable_label(layer.kind == LayerKind::Guide, "Guide").clicked() {
        set_kind(st, if layer.kind == LayerKind::Guide { LayerKind::Normal } else { LayerKind::Guide }, "Guide Layer");
        ui.close();
    }
    if ui.button("Add Classic Motion Guide").clicked() {
        st.checkpoint("Add Motion Guide");
        let name = format!("Guide: {}", layer.name);
        let gid = st.timeline_mut().add_layer(&name, li);
        st.timeline_mut().layer_mut(gid).unwrap().kind = LayerKind::Guide;
        if let Some(l) = st.timeline_mut().layer_mut(layer.id) {
            l.guide_layer = Some(gid);
        }
        st.layer = li;
        ui.close();
    }
    if ui.selectable_label(layer.kind == LayerKind::Mask, "Mask").clicked() {
        st.checkpoint("Mask Layer");
        let to_mask = layer.kind != LayerKind::Mask;
        let id = layer.id;
        let tl = st.timeline_mut();
        tl.layers[li].kind = if to_mask { LayerKind::Mask } else { LayerKind::Normal };
        tl.layers[li].locked = to_mask;
        // The layer directly below becomes masked.
        if to_mask && li + 1 < tl.layers.len() && tl.layers[li + 1].kind == LayerKind::Normal {
            tl.layers[li + 1].mask_layer = Some(id);
            tl.layers[li + 1].locked = true;
        }
        if !to_mask {
            for l in &mut tl.layers {
                if l.mask_layer == Some(id) {
                    l.mask_layer = None;
                }
            }
        }
        ui.close();
    }
    // Masked by the nearest mask layer above.
    let mask_above = st.timeline().layers[..li].iter().rev().find(|l| l.kind == LayerKind::Mask).map(|l| l.id);
    if let Some(mid) = mask_above
        && ui.selectable_label(layer.mask_layer.is_some(), "Masked").clicked()
    {
        st.checkpoint("Masked Layer");
        let l = &mut st.timeline_mut().layers[li];
        l.mask_layer = if l.mask_layer.is_some() { None } else { Some(mid) };
        ui.close();
    }
    ui.menu_button("Parent Layer", |ui| {
        if ui.selectable_label(layer.parent_layer.is_none(), "None").clicked() {
            st.checkpoint("Parent Layer");
            st.timeline_mut().layers[li].parent_layer = None;
            ui.close();
        }
        let others: Vec<(animara_document::Id, String)> =
            st.timeline().layers.iter().enumerate().filter(|(i, l)| *i != li && l.kind == LayerKind::Normal).map(|(_, l)| (l.id, l.name.clone())).collect();
        for (id, name) in others {
            if ui.selectable_label(layer.parent_layer == Some(id), name).clicked() {
                st.checkpoint("Parent Layer");
                st.timeline_mut().layers[li].parent_layer = Some(id);
                ui.close();
            }
        }
    });
    ui.menu_button("Layer Properties", |ui| {
        let mut color = crate::stage::c32(layer.color);
        if ui.color_edit_button_srgba(&mut color).changed() {
            st.touch();
            st.timeline_mut().layers[li].color = animara_document::Color::from_rgba8(color.r(), color.g(), color.b(), 255);
        }
        let mut depth = layer.depth;
        if ui.add(egui::DragValue::new(&mut depth).prefix("Layer depth: ").speed(1.0)).changed() {
            st.touch();
            st.timeline_mut().layers[li].depth = depth;
        }
        let mut outline = layer.outline;
        if ui.checkbox(&mut outline, "View as outlines").changed() {
            st.touch();
            st.timeline_mut().layers[li].outline = outline;
        }
    });
}
