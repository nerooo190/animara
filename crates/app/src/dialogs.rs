//! Modal dialogs and the command palette.

use crate::commands::Command as C;
use crate::core::{AppCore, Dialog, ExportKind};
use crate::{io_ops, ops};
use animara_document::{SymbolKind, TEMPLATES};
use egui::RichText;

const CATEGORIES: [&str; 7] = ["Character Animation", "Social", "Game", "Education", "Ads", "Web", "Advanced"];

fn window<'a>(title: &'a str, open: &'a mut bool) -> egui::Window<'a> {
    egui::Window::new(title).open(open).collapsible(false).resizable(true).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
}

pub fn dialogs(core: &mut AppCore, ctx: &egui::Context) {
    let Some(mut dialog) = core.dialog.take() else {
        palette(core, ctx);
        return;
    };
    let mut open = true;
    let mut close = false;
    match &mut dialog {
        Dialog::Welcome => {
            window("Welcome to Animara", &mut open).default_width(720.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(220.0);
                        ui.heading("Create new");
                        for t in TEMPLATES.iter().take(4) {
                            if ui.button(format!("{} — {}×{}", t.name, t.width, t.height)).clicked() {
                                io_ops::new_document(core, t.width, t.height, t.fps);
                                close = true;
                            }
                        }
                        if ui.button("More presets…").clicked() {
                            core.run(C::NewDocument);
                            close = true;
                        }
                        ui.separator();
                        if ui.button("📂 Open…").clicked() {
                            io_ops::open_dialog(core);
                            close = true;
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.heading("Recent");
                        let recent = core.settings.recent.clone();
                        if recent.is_empty() {
                            ui.weak("No recent documents.");
                        }
                        for p in recent {
                            if ui
                                .link(p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
                                .on_hover_text(p.display().to_string())
                                .clicked()
                            {
                                io_ops::open_path(core, &p);
                                close = true;
                            }
                        }
                        ui.separator();
                        ui.heading("Sample files");
                        for (name, f) in crate::samples::SAMPLES {
                            if ui.button(*name).clicked() {
                                let doc = f();
                                core.docs.push(crate::state::DocState::new(doc, None));
                                core.active = core.docs.len() - 1;
                                close = true;
                            }
                        }
                    });
                });
                ui.separator();
                ui.checkbox(&mut core.settings.show_welcome, "Show this screen on startup");
            });
        }
        Dialog::NewDocument { category, template, width, height, fps } => {
            window("New Document", &mut open).default_width(640.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, c) in CATEGORIES.iter().enumerate() {
                        if ui.selectable_label(*category == i, *c).clicked() {
                            *category = i;
                            *template = 0;
                            if let Some(t) = TEMPLATES.iter().find(|t| t.category == *c) {
                                (*width, *height, *fps) = (t.width, t.height, t.fps);
                            }
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(340.0);
                        let presets: Vec<_> = TEMPLATES.iter().filter(|t| t.category == CATEGORIES[*category]).collect();
                        ui.label(RichText::new(format!("PRESETS ({})", presets.len())).small().weak());
                        egui::Grid::new("presets").num_columns(3).spacing([8.0, 8.0]).show(ui, |ui| {
                            for (i, t) in presets.iter().enumerate() {
                                let text = format!("▷\n{}\n{}×{}", t.name, t.width, t.height);
                                if ui.add_sized([100.0, 70.0], egui::Button::selectable(*template == i, text)).clicked() {
                                    *template = i;
                                    (*width, *height, *fps) = (t.width, t.height, t.fps);
                                }
                                if i % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(RichText::new("DETAILS").small().weak());
                        egui::Grid::new("details").num_columns(2).show(ui, |ui| {
                            ui.label("Width");
                            ui.add(egui::DragValue::new(width).range(1.0..=8192.0));
                            ui.end_row();
                            ui.label("Height");
                            ui.add(egui::DragValue::new(height).range(1.0..=8192.0));
                            ui.end_row();
                            ui.label("Units");
                            ui.label("Pixels");
                            ui.end_row();
                            ui.label("Frame Rate");
                            ui.add(egui::DragValue::new(fps).range(0.01..=120.0).speed(0.1));
                            ui.end_row();
                            ui.label("Platform");
                            ui.label("Animara (Rhai scripting)");
                            ui.end_row();
                        });
                        ui.add_space(12.0);
                        if ui.add(egui::Button::new(RichText::new("Create").strong()).min_size(egui::vec2(100.0, 28.0))).clicked() {
                            io_ops::new_document(core, *width, *height, *fps);
                            close = true;
                        }
                    });
                });
            });
        }
        Dialog::DocumentSettings { width, height, fps, background, scale_content } => {
            window("Document Settings", &mut open).show(ctx, |ui| {
                egui::Grid::new("ds").num_columns(2).show(ui, |ui| {
                    ui.label("Stage size");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(width).range(1.0..=8192.0).suffix(" px"));
                        ui.label("×");
                        ui.add(egui::DragValue::new(height).range(1.0..=8192.0).suffix(" px"));
                    });
                    ui.end_row();
                    ui.label("Scale content");
                    ui.checkbox(scale_content, "with stage");
                    ui.end_row();
                    ui.label("Stage color");
                    let mut c = crate::stage::c32(*background);
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        *background = crate::panels::to_color(c);
                    }
                    ui.end_row();
                    ui.label("Frame rate");
                    ui.add(egui::DragValue::new(fps).range(0.01..=120.0).speed(0.1));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked()
                        && let Some(st) = core.doc_mut()
                    {
                        st.checkpoint("Document Settings");
                        if *scale_content {
                            let (sx, sy) = (*width / st.doc.width, *height / st.doc.height);
                            let m = animara_geometry::Affine::scale(sx, sy);
                            for sc in &mut st.doc.scenes {
                                for l in &mut sc.timeline.layers {
                                    for k in &mut l.frames {
                                        for e in &mut k.elements {
                                            e.transform = m * e.transform;
                                        }
                                    }
                                }
                            }
                        }
                        st.doc.width = *width;
                        st.doc.height = *height;
                        st.doc.fps = *fps;
                        st.doc.background = *background;
                        st.view.needs_fit = true;
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Dialog::ConvertToSymbol { .. } | Dialog::NewSymbol { .. } => {
            let is_convert = matches!(dialog, Dialog::ConvertToSymbol { .. });
            let mut center_v = if let Dialog::ConvertToSymbol { center, .. } = &dialog { *center } else { true };
            let (mut name_v, mut kind_v) = match &dialog {
                Dialog::ConvertToSymbol { name, kind, .. } | Dialog::NewSymbol { name, kind } => (name.clone(), *kind),
                _ => unreachable!(),
            };
            window(if is_convert { "Convert to Symbol" } else { "Create New Symbol" }, &mut open).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut name_v).request_focus();
                });
                ui.horizontal(|ui| {
                    ui.label("Type");
                    for k in [SymbolKind::MovieClip, SymbolKind::Graphic, SymbolKind::Button] {
                        ui.selectable_value(&mut kind_v, k, k.name());
                    }
                });
                if is_convert {
                    ui.horizontal(|ui| {
                        ui.label("Registration");
                        ui.selectable_value(&mut center_v, true, "Center");
                        ui.selectable_value(&mut center_v, false, "Top-left");
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let auto_key = core.settings.tools.auto_keyframe;
                        if let Some(st) = core.doc_mut() {
                            if is_convert {
                                ops::convert_to_symbol(st, &name_v, kind_v, center_v, auto_key);
                            } else {
                                ops::new_symbol(st, &name_v, kind_v);
                            }
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
            dialog = if is_convert {
                Dialog::ConvertToSymbol { name: name_v, kind: kind_v, center: center_v }
            } else {
                Dialog::NewSymbol { name: name_v, kind: kind_v }
            };
        }
        Dialog::Export(s) => {
            let total = core.doc().map(|d| d.doc.total_frames()).unwrap_or(1);
            window("Export", &mut open).default_width(420.0).show(ctx, |ui| {
                egui::ComboBox::from_label("Format").selected_text(s.kind.name()).width(260.0).show_ui(ui, |ui| {
                    for k in ExportKind::ALL {
                        ui.selectable_value(&mut s.kind, k, k.name());
                    }
                });
                let video = matches!(s.kind, ExportKind::Mp4 | ExportKind::WebM | ExportKind::Mov | ExportKind::WebP);
                if video && !animara_media::ffmpeg_available() {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xe0, 0x80, 0x30),
                        "ffmpeg was not found on PATH — install FFmpeg to export this format.",
                    );
                }
                egui::Grid::new("exp").num_columns(2).show(ui, |ui| {
                    ui.label("Scale");
                    ui.add(egui::Slider::new(&mut s.scale, 0.1..=4.0).suffix("×"));
                    ui.end_row();
                    if !matches!(s.kind, ExportKind::Png | ExportKind::Svg | ExportKind::Xfl) {
                        ui.label("Frames");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut s.first).range(1..=total));
                            ui.label("to");
                            ui.add(egui::DragValue::new(&mut s.last).range(s.first..=total));
                        });
                        ui.end_row();
                        ui.label("Frame rate");
                        ui.add(egui::DragValue::new(&mut s.fps).range(0.01..=120.0));
                        ui.end_row();
                    }
                    ui.label("Background");
                    ui.checkbox(&mut s.transparent, "Transparent");
                    ui.end_row();
                    ui.label("Quality");
                    ui.add(egui::Slider::new(&mut s.quality, 1..=100));
                    ui.end_row();
                    if matches!(s.kind, ExportKind::Gif | ExportKind::Apng | ExportKind::WebP) {
                        ui.label("Repeat");
                        ui.horizontal(|ui| {
                            let mut forever = s.loop_count == 0;
                            if ui.checkbox(&mut forever, "Forever").changed() {
                                s.loop_count = if forever { 0 } else { 1 };
                            }
                            if !forever {
                                ui.add(egui::DragValue::new(&mut s.loop_count).range(1..=1000).suffix(" times"));
                            }
                        });
                        ui.end_row();
                    }
                    if video || matches!(s.kind, ExportKind::Html5 | ExportKind::Swf) {
                        ui.label("Audio");
                        ui.checkbox(&mut s.include_audio, "Include timeline audio");
                        ui.end_row();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Export…").strong()).clicked() {
                        let settings = s.clone();
                        io_ops::run_export(core, &settings);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Dialog::Preferences => {
            window("Preferences", &mut open).show(ctx, |ui| {
                let s = &mut core.settings;
                ui.heading("General");
                ui.checkbox(&mut s.dark, "Dark theme");
                ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=2.0).text("Interface scale (HiDPI)"));
                ui.checkbox(&mut s.show_welcome, "Show welcome screen on startup");
                ui.add(egui::Slider::new(&mut s.autosave_secs, 0..=900).text("Autosave interval (s, 0 = off)"));
                ui.heading("Drawing");
                ui.checkbox(&mut s.tools.snap_objects, "Snap to objects");
                ui.checkbox(&mut s.tools.auto_keyframe, "Auto keyframe");
                ui.checkbox(&mut s.pasteboard_content, "Show content on the pasteboard");
                ui.heading("Timeline");
                ui.add(egui::Slider::new(&mut s.frame_width, 4.0..=40.0).text("Frame width"));
                ui.checkbox(&mut s.mute, "Mute sounds");
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            crate::theme::apply(ctx, &core.settings);
        }
        Dialog::Shortcuts { filter, capturing } => {
            window("Keyboard Shortcuts", &mut open).default_height(520.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    ui.text_edit_singleline(filter);
                    if ui.button("Reset all to defaults").clicked() {
                        core.settings.shortcuts.clear();
                    }
                });
                if let Some(c) = *capturing {
                    ui.label(RichText::new(format!("Press the new shortcut for “{}” (Esc to clear)…", c.label())).strong());
                    let ev = ui.input(|i| {
                        i.events.iter().find_map(|e| {
                            if let egui::Event::Key { key, pressed: true, modifiers, .. } = e { Some((*key, *modifiers)) } else { None }
                        })
                    });
                    if let Some((key, mods)) = ev {
                        if key == egui::Key::Escape {
                            core.settings.shortcuts.insert(c.id().into(), String::new());
                        } else {
                            let sc = egui::KeyboardShortcut::new(mods, key);
                            // Remove the binding from any other command using it.
                            let text = ui.ctx().format_shortcut(&sc);
                            for other in C::ALL {
                                if *other != c && core.settings.shortcut(*other).is_some_and(|o| o == sc) {
                                    core.settings.shortcuts.insert(other.id().into(), String::new());
                                }
                            }
                            let mut parts = Vec::new();
                            if mods.command {
                                parts.push("Ctrl".to_string());
                            }
                            if mods.alt {
                                parts.push("Alt".into());
                            }
                            if mods.shift {
                                parts.push("Shift".into());
                            }
                            parts.push(key.name().to_string());
                            core.settings.shortcuts.insert(c.id().into(), parts.join("+"));
                            core.info(format!("Bound {} to {}", c.label(), text));
                        }
                        *capturing = None;
                    }
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    egui::Grid::new("sc").striped(true).num_columns(3).show(ui, |ui| {
                        for c in C::ALL {
                            if !filter.is_empty() && !c.label().to_lowercase().contains(&filter.to_lowercase()) {
                                continue;
                            }
                            ui.weak(c.category());
                            ui.label(c.label());
                            let text = core.settings.shortcut(*c).map(|s| ui.ctx().format_shortcut(&s)).unwrap_or_else(|| "—".into());
                            if ui.button(text).clicked() {
                                *capturing = Some(*c);
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        }
        Dialog::About => {
            window("About Animara", &mut open).show(ctx, |ui| {
                ui.heading("Animara");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A 2D vector animation studio written in Rust.");
                ui.label("Independent software; not affiliated with or endorsed by Adobe. Adobe Animate is a trademark of Adobe Inc.");
                ui.separator();
                ui.label(format!(
                    "FFmpeg: {}",
                    if animara_media::ffmpeg_available() { "available" } else { "not found (video import/export disabled)" }
                ));
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        }
        Dialog::SwapSymbol { selected } => {
            window("Swap Symbol", &mut open).show(ctx, |ui| {
                if let Some(st) = core.doc() {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for s in &st.doc.library.symbols {
                            if ui.selectable_label(*selected == Some(s.id), format!("◆ {} ({})", s.name, s.kind.name())).clicked() {
                                *selected = Some(s.id);
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        let auto_key = core.settings.tools.auto_keyframe;
                        if let (Some(id), Some(st)) = (*selected, core.doc_mut()) {
                            ops::swap_symbol(st, id, auto_key);
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        }
        Dialog::SaveWorkspace { name } => {
            window("New Workspace", &mut open).show(ctx, |ui| {
                ui.text_edit_singleline(name);
                if ui.button("Save").clicked() {
                    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("save_workspace"), name.clone()));
                    close = true;
                }
            });
        }
        Dialog::ConfirmClose { uid, then_exit } => {
            let uid = *uid;
            let then_exit = *then_exit;
            let name = core.docs.iter().find(|d| d.uid == uid).map(|d| d.doc.name.clone());
            match name {
                None => close = true,
                Some(name) => {
                    window("Save changes?", &mut open).show(ctx, |ui| {
                        ui.label(format!("Save changes to “{name}” before closing?"));
                        ui.horizontal(|ui| {
                            let mut next = false;
                            if ui.button("Save").clicked() {
                                core.active = core.docs.iter().position(|d| d.uid == uid).unwrap_or(0);
                                if io_ops::save(core, false) {
                                    crate::menus::close_doc(core, uid);
                                    next = true;
                                }
                            }
                            if ui.button("Don't Save").clicked() {
                                crate::menus::close_doc(core, uid);
                                next = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                            if next {
                                close = true;
                                if let Some(d) = core.docs.iter().find(|d| d.dirty) {
                                    core.dialog = Some(Dialog::ConfirmClose { uid: d.uid, then_exit });
                                } else if then_exit {
                                    core.exit_requested = true;
                                }
                            }
                        });
                    });
                }
            }
            if close && core.dialog.is_some() {
                return;
            }
        }
        Dialog::Recovery { files } => {
            window("Recover documents", &mut open).show(ctx, |ui| {
                ui.label("Animara did not shut down cleanly last time. These documents were autosaved:");
                let mut remove = None;
                for (i, f) in files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                        if ui.button("Restore").clicked() {
                            match animara_document::load_document(f) {
                                Ok(mut d) => {
                                    d.name = format!("{} (recovered)", d.name);
                                    let mut st = crate::state::DocState::new(d, None);
                                    st.dirty = true;
                                    core.docs.push(st);
                                    core.active = core.docs.len() - 1;
                                }
                                Err(e) => core.error(format!("Recovery failed: {e}")),
                            }
                            let _ = std::fs::remove_file(f);
                            remove = Some(i);
                        }
                        if ui.button("Discard").clicked() {
                            let _ = std::fs::remove_file(f);
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    files.remove(i);
                }
                if files.is_empty() {
                    close = true;
                }
            });
        }
        Dialog::Plugins => {
            window("Commands & Plugins", &mut open).show(ctx, |ui| {
                let dir = crate::core::Settings::config_dir().join("plugins");
                ui.label(format!("Plugin folder: {}", dir.display()));
                ui.label("Plugins are sandboxed Rhai scripts defining `fn run(doc)`; they cannot access files or the network.");
                if ui.button("Reload plugins").clicked() {
                    core.plugins = animara_scripting::load_plugins(&dir);
                }
                ui.separator();
                let plugins = core.plugins.clone();
                for p in plugins {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&p.name).strong());
                        ui.weak(&p.description);
                        if ui.button("Run").clicked() {
                            crate::menus::run_plugin(core, &p);
                        }
                    });
                }
            });
        }
        Dialog::Help => {
            window("Animara Help", &mut open).default_width(520.0).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    ui.label(RichText::new(crate::samples::HELP_TEXT).monospace());
                });
            });
        }
    }
    if open && !close {
        core.dialog = Some(dialog);
    }
}

/// Searchable command palette (Ctrl+Shift+P).
fn palette(core: &mut AppCore, ctx: &egui::Context) {
    let Some(mut q) = core.palette.take() else { return };
    let mut run = None;
    let mut keep = true;
    egui::Window::new("Command Palette").title_bar(false).anchor(egui::Align2::CENTER_TOP, [0.0, 60.0]).fixed_size([460.0, 360.0]).show(
        ctx,
        |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Type a command…").desired_width(f32::INFINITY));
            r.request_focus();
            let ql = q.to_lowercase();
            let matches: Vec<C> = C::ALL
                .iter()
                .copied()
                .filter(|c| ql.split_whitespace().all(|w| c.label().to_lowercase().contains(w) || c.category().to_lowercase().contains(w)))
                .take(14)
                .collect();
            for (i, c) in matches.iter().enumerate() {
                let sc = core.settings.shortcut(*c).map(|s| ctx.format_shortcut(&s)).unwrap_or_default();
                if ui.add(egui::Button::selectable(i == 0, format!("{} › {}", c.category(), c.label())).shortcut_text(sc)).clicked() {
                    run = Some(*c);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                run = matches.first().copied();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        },
    );
    if let Some(c) = run {
        core.run(c);
        keep = false;
    }
    if keep {
        core.palette = Some(q);
    }
}
