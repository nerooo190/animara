//! Menu bar, keyboard shortcuts and command execution.

use crate::commands::Command as C;
use crate::core::{AppCore, Dialog, ExportKind};
use crate::io_ops;
use crate::ops;
use crate::tools::Tool;
use animara_document::{LayerKind, SymbolKind, TweenKind};
use animara_geometry::{Affine, BoolOp, Vec2};

/// Actions that need the window/dock layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    ShowPanel(crate::app::Tab),
    ResetWorkspace,
    TogglePanels,
}

/// Process shortcuts (skipped while a text field has keyboard focus).
pub fn handle_shortcuts(core: &mut AppCore, ctx: &egui::Context) {
    let typing = ctx.egui_wants_keyboard_input() || core.doc().is_some_and(|d| d.text_edit.is_some());
    for &c in C::ALL {
        let Some(sc) = core.settings.shortcut(c) else { continue };
        // Bare keys (no modifiers) never fire while typing.
        let bare = !sc.modifiers.command && !sc.modifiers.alt && !sc.modifiers.ctrl;
        if typing && (bare || sc.modifiers.shift && !sc.modifiers.command) {
            continue;
        }
        if typing && matches!(c, C::Undo | C::Redo | C::Copy | C::Cut | C::Paste | C::SelectAll) {
            continue;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc)) {
            core.run(c);
        }
    }
}

fn enabled(core: &AppCore, c: C) -> bool {
    let has_doc = core.doc().is_some();
    let has_sel = core.doc().is_some_and(|d| !d.selection.is_empty());
    match c {
        C::NewDocument | C::NewFromTemplate | C::Open | C::Exit | C::Preferences | C::KeyboardShortcuts | C::About | C::ShowHelp | C::CommandPalette => true,
        C::ToggleTheme | C::ResetWorkspace | C::SaveWorkspace | C::HidePanels | C::ManagePlugins | C::RunScriptCommand => true,
        C::Undo => core.doc().is_some_and(|d| d.history.can_undo()),
        C::Redo => core.doc().is_some_and(|d| d.history.can_redo()),
        C::Cut | C::Copy | C::Clear | C::Duplicate | C::ConvertToSymbol | C::BreakApart | C::Group | C::Ungroup | C::FreeTransform
        | C::Rotate90CW | C::Rotate90CCW | C::FlipHorizontal | C::FlipVertical | C::RemoveTransform | C::BringToFront | C::BringForward
        | C::SendBackward | C::SendToBack | C::LockSelection | C::DistributeToLayers | C::Smooth | C::Straighten | C::Optimize
        | C::LinesToFills | C::Union | C::Intersect | C::Punch | C::Crop | C::SwapSymbol | C::EditSelected | C::EditInPlace
        | C::AlignLeft | C::AlignHCenter | C::AlignRight | C::AlignTop | C::AlignVCenter | C::AlignBottom | C::DistributeH | C::DistributeV
        | C::MatchWidth | C::MatchHeight | C::TextBold | C::TextItalic | C::TextAlignLeft | C::TextAlignCenter | C::TextAlignRight
        | C::TextToOutlines => has_sel,
        C::ExitSymbol => core.doc().is_some_and(|d| !d.stack.is_empty()),
        C::Paste | C::PasteInPlace => has_doc && !core.clipboard.elements.is_empty(),
        C::DuplicateSymbol => core.library_selected.is_some(),
        _ => has_doc,
    }
}

fn item(core: &mut AppCore, ui: &mut egui::Ui, c: C) {
    let sc = core.settings.shortcut(c).map(|s| ui.ctx().format_shortcut(&s)).unwrap_or_default();
    let checked = checked(core, c);
    let label = match checked {
        Some(true) => format!("✔ {}", c.label()),
        Some(false) => format!("   {}", c.label()),
        None => c.label().to_string(),
    };
    if ui.add_enabled(enabled(core, c), egui::Button::new(label).shortcut_text(sc)).clicked() {
        core.run(c);
        ui.close();
    }
}

fn checked(core: &AppCore, c: C) -> Option<bool> {
    let d = core.doc();
    Some(match c {
        C::ToggleRulers => core.settings.rulers,
        C::ToggleGrid => d.is_some_and(|d| d.doc.grid.visible),
        C::ToggleSnapGrid => d.is_some_and(|d| d.doc.grid.snap),
        C::ToggleGuides => d.is_some_and(|d| d.doc.guides.visible),
        C::ToggleSnapGuides => d.is_some_and(|d| d.doc.guides.snap),
        C::ToggleSnapObjects => core.settings.tools.snap_objects,
        C::TogglePreviewOutlines => core.settings.preview_outlines,
        C::TogglePasteboard => core.settings.pasteboard_content,
        C::ToggleLoop => d.is_some_and(|d| d.loop_playback),
        C::ToggleOnionSkin => d.is_some_and(|d| d.onion.enabled),
        C::ToggleOnionOutline => d.is_some_and(|d| d.onion.outline),
        C::MuteSounds => core.settings.mute,
        C::ToggleAlignToStage => core.settings.tools.align_to_stage,
        _ => return None,
    })
}

pub fn menu_bar(core: &mut AppCore, ui: &mut egui::Ui) {
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            for c in [C::NewDocument, C::NewFromTemplate, C::Open] {
                item(core, ui, c);
            }
            ui.menu_button("Open Recent", |ui| {
                let recent = core.settings.recent.clone();
                if recent.is_empty() {
                    ui.weak("(empty)");
                }
                for p in recent {
                    if ui.button(p.display().to_string()).clicked() {
                        io_ops::open_path(core, &p);
                        ui.close();
                    }
                }
            });
            ui.separator();
            for c in [C::Close, C::CloseAll, C::Save, C::SaveAs, C::SaveAll, C::Revert] {
                item(core, ui, c);
            }
            ui.separator();
            ui.menu_button("Import", |ui| {
                for c in [C::ImportToStage, C::ImportToLibrary, C::OpenExternalLibrary] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Export", |ui| {
                for c in [C::ExportImage, C::ExportAnimatedGif, C::ExportVideo, C::ExportMovie, C::ExportSvg, C::ExportSpriteSheet, C::ExportLottie, C::ExportHtml5, C::ExportSwf, C::ExportXfl] {
                    item(core, ui, c);
                }
            });
            ui.separator();
            for c in [C::PublishSettings, C::Publish] {
                item(core, ui, c);
            }
            ui.separator();
            item(core, ui, C::Exit);
        });
        ui.menu_button("Edit", |ui| {
            let undo_label = core.doc().and_then(|d| d.history.undo_labels().last().map(String::from));
            if let Some(l) = undo_label {
                ui.weak(format!("Undo: {l}"));
            }
            for c in [C::Undo, C::Redo] {
                item(core, ui, c);
            }
            ui.separator();
            for c in [C::Cut, C::Copy, C::Paste, C::PasteInPlace, C::Clear, C::Duplicate] {
                item(core, ui, c);
            }
            ui.separator();
            for c in [C::SelectAll, C::DeselectAll] {
                item(core, ui, c);
            }
            ui.separator();
            ui.menu_button("Timeline", |ui| {
                for c in [C::InsertFrame, C::RemoveFrames, C::InsertKeyframe, C::InsertBlankKeyframe, C::ClearKeyframe, C::ConvertToKeyframes, C::ReverseFrames] {
                    item(core, ui, c);
                }
            });
            ui.separator();
            for c in [C::EditSymbols, C::EditSelected, C::EditInPlace, C::ExitSymbol] {
                item(core, ui, c);
            }
            ui.separator();
            for c in [C::CommandPalette, C::Preferences, C::KeyboardShortcuts] {
                item(core, ui, c);
            }
        });
        ui.menu_button("View", |ui| {
            for c in [C::ZoomIn, C::ZoomOut, C::Zoom100, C::FitInWindow, C::ShowFrame] {
                item(core, ui, c);
            }
            ui.separator();
            ui.menu_button("Preview Mode", |ui| {
                for c in [C::TogglePreviewOutlines, C::TogglePasteboard] {
                    item(core, ui, c);
                }
            });
            ui.separator();
            for c in [C::ToggleRulers, C::ToggleGrid, C::ToggleSnapGrid, C::ToggleGuides, C::ToggleSnapGuides, C::ClearGuides, C::ToggleSnapObjects] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Insert", |ui| {
            item(core, ui, C::NewSymbol);
            ui.menu_button("Timeline", |ui| {
                for c in [C::InsertLayer, C::InsertFolder, C::InsertCameraLayer, C::InsertFrame, C::InsertKeyframe, C::InsertBlankKeyframe] {
                    item(core, ui, c);
                }
            });
            for c in [C::CreateMotionTween, C::CreateShapeTween, C::CreateClassicTween, C::RemoveTween, C::InsertScene] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Modify", |ui| {
            for c in [C::DocumentSettings, C::ConvertToSymbol, C::BreakApart] {
                item(core, ui, c);
            }
            ui.menu_button("Symbol", |ui| {
                for c in [C::SwapSymbol, C::DuplicateSymbol] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Shape", |ui| {
                for c in [C::Smooth, C::Straighten, C::Optimize, C::LinesToFills] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Combine Objects", |ui| {
                for c in [C::Union, C::Intersect, C::Punch, C::Crop] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Timeline", |ui| {
                for c in [C::DistributeToLayers, C::ReverseFrames, C::ConvertToKeyframes] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Transform", |ui| {
                for c in [C::FreeTransform, C::Rotate90CW, C::Rotate90CCW, C::FlipHorizontal, C::FlipVertical, C::RemoveTransform] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Arrange", |ui| {
                for c in [C::BringToFront, C::BringForward, C::SendBackward, C::SendToBack, C::LockSelection, C::UnlockAll] {
                    item(core, ui, c);
                }
            });
            ui.menu_button("Align", |ui| {
                for c in [C::AlignLeft, C::AlignHCenter, C::AlignRight, C::AlignTop, C::AlignVCenter, C::AlignBottom, C::DistributeH, C::DistributeV, C::MatchWidth, C::MatchHeight, C::ToggleAlignToStage] {
                    item(core, ui, c);
                }
            });
            for c in [C::Group, C::Ungroup] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Text", |ui| {
            for c in [C::TextBold, C::TextItalic, C::TextAlignLeft, C::TextAlignCenter, C::TextAlignRight, C::TextToOutlines] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Commands", |ui| {
            item(core, ui, C::ManagePlugins);
            item(core, ui, C::RunScriptCommand);
            ui.separator();
            let plugins = core.plugins.clone();
            for p in plugins {
                if ui.button(&p.name).on_hover_text(&p.description).clicked() {
                    run_plugin(core, &p);
                    ui.close();
                }
            }
        });
        ui.menu_button("Control", |ui| {
            for c in [C::PlayStop, C::Rewind, C::GoToEnd, C::StepForward, C::StepBackward, C::NextKeyframe, C::PrevKeyframe] {
                item(core, ui, c);
            }
            ui.separator();
            item(core, ui, C::TestMovie);
            ui.separator();
            for c in [C::ToggleLoop, C::ToggleOnionSkin, C::ToggleOnionOutline, C::MuteSounds] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Debug", |ui| {
            for c in [C::DebugMovie, C::ValidateDocument] {
                item(core, ui, c);
            }
        });
        ui.menu_button("Window", |ui| {
            for c in [C::ShowTimeline, C::ShowProperties, C::ShowLibrary, C::ShowAssets, C::ShowColor, C::ShowSwatches, C::ShowAlign, C::ShowTransform, C::ShowHistory, C::ShowActions, C::ShowOutput, C::ShowMotionEditor, C::ShowScenes, C::ShowInfo, C::ShowComponents] {
                item(core, ui, c);
            }
            ui.separator();
            ui.menu_button("Workspaces", |ui| {
                let names: Vec<String> = core.settings.workspaces.keys().cloned().collect();
                for n in names {
                    if ui.selectable_label(core.settings.current_workspace == n, &n).clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("load_workspace"), n.clone()));
                        ui.close();
                    }
                }
                ui.separator();
                for c in [C::SaveWorkspace, C::ResetWorkspace] {
                    item(core, ui, c);
                }
            });
            for c in [C::ToggleTheme, C::HidePanels] {
                item(core, ui, c);
            }
            ui.separator();
            let titles: Vec<(usize, String)> = core.docs.iter().enumerate().map(|(i, d)| (i, d.title())).collect();
            for (i, t) in titles {
                if ui.selectable_label(i == core.active, t).clicked() {
                    core.active = i;
                    ui.close();
                }
            }
        });
        ui.menu_button("Help", |ui| {
            for c in [C::ShowHelp, C::KeyboardShortcuts, C::About] {
                item(core, ui, c);
            }
        });
    });
}

pub fn run_plugin(core: &mut AppCore, p: &animara_scripting::Plugin) {
    let Some(st) = core.doc_mut() else {
        core.warn("Open a document before running a command.");
        return;
    };
    match animara_scripting::run_plugin(p, &st.doc) {
        Ok((doc, log)) => {
            st.checkpoint(&format!("Command: {}", p.name));
            st.doc = doc;
            st.clamp();
            for l in log {
                core.info(format!("[{}] {l}", p.name));
            }
            core.info(format!("Command “{}” finished.", p.name));
        }
        Err(e) => core.error(format!("Command “{}” failed: {e}", p.name)),
    }
}

/// Execute a command. Returns a UI-level action when the dock must change.
pub fn execute(core: &mut AppCore, c: C, ctx: &egui::Context) -> Option<UiAction> {
    use crate::app::Tab;
    let auto_key = core.settings.tools.auto_keyframe;
    let align_stage = core.settings.tools.align_to_stage;
    macro_rules! doc {
        () => {
            match core.docs.get_mut(core.active) {
                Some(d) => d,
                None => return None,
            }
        };
    }
    match c {
        C::NewDocument => core.dialog = Some(Dialog::NewDocument { category: 0, template: 0, width: 1280.0, height: 720.0, fps: 30.0 }),
        C::NewFromTemplate => core.dialog = Some(Dialog::Welcome),
        C::Open => io_ops::open_dialog(core),
        C::Close => {
            if let Some((uid, dirty)) = core.doc().map(|d| (d.uid, d.dirty)) {
                if dirty {
                    core.dialog = Some(Dialog::ConfirmClose { uid, then_exit: false });
                } else {
                    close_doc(core, uid);
                }
            }
        }
        C::CloseAll => {
            let uids: Vec<u64> = core.docs.iter().filter(|d| !d.dirty).map(|d| d.uid).collect();
            for u in uids {
                close_doc(core, u);
            }
            if let Some(d) = core.docs.first() {
                core.dialog = Some(Dialog::ConfirmClose { uid: d.uid, then_exit: false });
            }
        }
        C::Save => {
            io_ops::save(core, false);
        }
        C::SaveAs => {
            io_ops::save(core, true);
        }
        C::SaveAll => {
            let n = core.docs.len();
            let cur = core.active;
            for i in 0..n {
                core.active = i;
                if core.docs[i].dirty {
                    io_ops::save(core, false);
                }
            }
            core.active = cur;
        }
        C::Revert => {
            if let Some(p) = core.doc().and_then(|d| d.path.clone()) {
                match animara_document::load_document(&p) {
                    Ok(d) => {
                        let st = doc!();
                        st.checkpoint("Revert");
                        st.doc = d;
                        st.dirty = false;
                        st.clamp();
                    }
                    Err(e) => core.error(format!("Revert failed: {e}")),
                }
            }
        }
        C::ImportToStage => io_ops::import(core, true),
        C::ImportToLibrary => io_ops::import(core, false),
        C::OpenExternalLibrary => {
            if let Some(p) = rfd::FileDialog::new().add_filter("Documents", &["anim", "fla", "xfl"]).pick_file() {
                io_ops::open_external_library(core, &p);
            }
        }
        C::ExportImage | C::ExportAnimatedGif | C::ExportVideo | C::ExportMovie | C::ExportSvg | C::ExportSpriteSheet | C::ExportLottie | C::ExportHtml5
        | C::ExportSwf | C::ExportXfl | C::PublishSettings => {
            let kind = match c {
                C::ExportImage => ExportKind::Png,
                C::ExportAnimatedGif => ExportKind::Gif,
                C::ExportVideo => ExportKind::Mp4,
                C::ExportMovie => ExportKind::PngSequence,
                C::ExportSvg => ExportKind::Svg,
                C::ExportSpriteSheet => ExportKind::SpriteSheet,
                C::ExportLottie => ExportKind::Lottie,
                C::ExportSwf => ExportKind::Swf,
                C::ExportXfl => ExportKind::Xfl,
                _ => ExportKind::Html5,
            };
            if let Some(st) = core.doc() {
                core.dialog = Some(Dialog::Export(io_ops::default_export(st, kind)));
            }
        }
        C::Publish => io_ops::publish(core),
        C::Exit => core.exit_requested = true,
        C::Undo => {
            let st = doc!();
            if let Some(l) = st.undo() {
                core.status = format!("Undo {l}");
            }
        }
        C::Redo => {
            let st = doc!();
            if let Some(l) = st.redo() {
                core.status = format!("Redo {l}");
            }
        }
        C::Cut => {
            ops::copy(doc!(), &mut core.clipboard);
            ops::delete_selection(doc!());
        }
        C::Copy => ops::copy(doc!(), &mut core.clipboard),
        C::Paste | C::PasteInPlace => {
            let clip = core.clipboard.clone();
            ops::paste(doc!(), &clip, c == C::PasteInPlace, auto_key);
        }
        C::Clear => ops::delete_selection(doc!()),
        C::Duplicate => ops::duplicate(doc!(), auto_key),
        C::SelectAll => ops::select_all(doc!()),
        C::DeselectAll => doc!().selection.clear(),
        C::EditSymbols => {
            let sel = core.library_selected;
            let st = doc!();
            if let Some(id) = sel.filter(|id| st.doc.library.symbol(*id).is_some()).or_else(|| st.doc.library.symbols.first().map(|s| s.id)) {
                st.stack.clear();
                ops::enter_symbol(st, id, None);
            }
        }
        C::EditSelected | C::EditInPlace => {
            let st = doc!();
            if let Some((_, e)) = st.selected().into_iter().find(|(_, e)| e.as_instance().is_some()) {
                let id = e.as_instance().unwrap().symbol;
                ops::enter_symbol(st, id, (c == C::EditInPlace).then_some(e.transform));
            }
        }
        C::ExitSymbol => {
            let st = doc!();
            let l = st.stack.len().saturating_sub(1);
            ops::exit_to_level(st, l);
        }
        C::Preferences => core.dialog = Some(Dialog::Preferences),
        C::KeyboardShortcuts => core.dialog = Some(Dialog::Shortcuts { filter: String::new(), capturing: None }),
        C::CommandPalette => core.palette = Some(String::new()),
        C::ZoomIn | C::ZoomOut => {
            let st = doc!();
            let f = if c == C::ZoomIn { 1.5 } else { 1.0 / 1.5 };
            let center = Vec2::new(st.view.pan.x + st.doc.width * st.view.zoom / 2.0, st.view.pan.y + st.doc.height * st.view.zoom / 2.0);
            crate::stage::zoom_about(st, f, center);
        }
        C::Zoom100 => {
            let st = doc!();
            st.view.zoom = 1.0;
        }
        C::FitInWindow | C::ShowFrame => doc!().view.needs_fit = true,
        C::ToggleRulers => core.settings.rulers = !core.settings.rulers,
        C::ToggleGrid | C::ToggleSnapGrid | C::ToggleGuides | C::ToggleSnapGuides | C::ClearGuides => {
            let st = doc!();
            st.checkpoint(c.label());
            match c {
                C::ToggleGrid => st.doc.grid.visible = !st.doc.grid.visible,
                C::ToggleSnapGrid => st.doc.grid.snap = !st.doc.grid.snap,
                C::ToggleGuides => st.doc.guides.visible = !st.doc.guides.visible,
                C::ToggleSnapGuides => st.doc.guides.snap = !st.doc.guides.snap,
                _ => {
                    st.doc.guides.horizontal.clear();
                    st.doc.guides.vertical.clear();
                }
            }
        }
        C::ToggleSnapObjects => core.settings.tools.snap_objects = !core.settings.tools.snap_objects,
        C::TogglePreviewOutlines => core.settings.preview_outlines = !core.settings.preview_outlines,
        C::TogglePasteboard => core.settings.pasteboard_content = !core.settings.pasteboard_content,
        C::NewSymbol => core.dialog = Some(Dialog::NewSymbol { name: "Symbol 1".into(), kind: SymbolKind::MovieClip }),
        C::InsertLayer => ops::add_layer(doc!(), LayerKind::Normal),
        C::InsertFolder => ops::add_layer(doc!(), LayerKind::Folder),
        C::InsertCameraLayer => ops::add_layer(doc!(), LayerKind::Camera),
        C::InsertFrame => ops::insert_frames(doc!()),
        C::RemoveFrames => ops::remove_frames(doc!()),
        C::InsertKeyframe => ops::insert_keyframe(doc!(), false),
        C::InsertBlankKeyframe => ops::insert_keyframe(doc!(), true),
        C::ClearKeyframe => ops::clear_keyframe(doc!()),
        C::CreateClassicTween => ops::create_tween(doc!(), Some(TweenKind::Classic)),
        C::CreateMotionTween => ops::create_tween(doc!(), Some(TweenKind::Motion)),
        C::CreateShapeTween => ops::create_tween(doc!(), Some(TweenKind::Shape)),
        C::RemoveTween => ops::create_tween(doc!(), None),
        C::InsertScene => ops::add_scene(doc!()),
        C::DocumentSettings => {
            let st = doc!();
            core.dialog = Some(Dialog::DocumentSettings { width: st.doc.width, height: st.doc.height, fps: st.doc.fps, background: st.doc.background, scale_content: false });
        }
        C::ConvertToSymbol => {
            let n = core.doc().map(|d| d.doc.library.unique_name("Symbol 1")).unwrap_or_default();
            core.dialog = Some(Dialog::ConvertToSymbol { name: n, kind: SymbolKind::MovieClip, center: true });
        }
        C::BreakApart | C::Ungroup => ops::break_apart(doc!(), auto_key),
        C::Group => ops::group(doc!(), auto_key),
        C::SwapSymbol => core.dialog = Some(Dialog::SwapSymbol { selected: None }),
        C::DuplicateSymbol => {
            if let Some(id) = core.library_selected {
                let new = ops::duplicate_symbol(doc!(), id);
                core.library_selected = new;
            }
        }
        C::Smooth => ops::reshape(doc!(), 0, auto_key),
        C::Straighten => ops::reshape(doc!(), 1, auto_key),
        C::Optimize => ops::reshape(doc!(), 2, auto_key),
        C::LinesToFills => ops::reshape(doc!(), 3, auto_key),
        C::Union => ops::combine(doc!(), BoolOp::Union, auto_key),
        C::Intersect => ops::combine(doc!(), BoolOp::Intersect, auto_key),
        C::Punch => ops::combine(doc!(), BoolOp::Difference, auto_key),
        C::Crop => ops::crop(doc!(), auto_key),
        C::ReverseFrames => ops::reverse_frames(doc!()),
        C::ConvertToKeyframes => ops::convert_to_keyframes(doc!()),
        C::DistributeToLayers => ops::distribute_to_layers(doc!()),
        C::FreeTransform => core.tool = Tool::FreeTransform,
        C::Rotate90CW | C::Rotate90CCW | C::FlipHorizontal | C::FlipVertical => {
            let st = doc!();
            let pivot = st.selection_bounds().center();
            let m = match c {
                C::Rotate90CW => Affine::rotate(std::f64::consts::FRAC_PI_2),
                C::Rotate90CCW => Affine::rotate(-std::f64::consts::FRAC_PI_2),
                C::FlipHorizontal => Affine::scale(-1.0, 1.0),
                _ => Affine::scale(1.0, -1.0),
            };
            ops::transform_selection_about(st, m, pivot, Some(c.label()), auto_key);
        }
        C::RemoveTransform => ops::remove_transform(doc!(), auto_key),
        C::BringToFront => ops::arrange(doc!(), true, true, auto_key),
        C::BringForward => ops::arrange(doc!(), true, false, auto_key),
        C::SendBackward => ops::arrange(doc!(), false, false, auto_key),
        C::SendToBack => ops::arrange(doc!(), false, true, auto_key),
        C::LockSelection => {
            ops::modify_selection(doc!(), Some("Lock"), auto_key, |e| e.locked = true);
            doc!().selection.clear();
        }
        C::UnlockAll => {
            let st = doc!();
            st.checkpoint("Unlock All");
            let f = st.frame;
            for l in &mut st.timeline_mut().layers {
                if let Some(k) = l.keyframe_at_mut(f) {
                    k.elements.iter_mut().for_each(|e| e.locked = false);
                }
            }
        }
        C::AlignLeft => ops::align(doc!(), 0, align_stage, auto_key),
        C::AlignHCenter => ops::align(doc!(), 1, align_stage, auto_key),
        C::AlignRight => ops::align(doc!(), 2, align_stage, auto_key),
        C::AlignTop => ops::align(doc!(), 3, align_stage, auto_key),
        C::AlignVCenter => ops::align(doc!(), 4, align_stage, auto_key),
        C::AlignBottom => ops::align(doc!(), 5, align_stage, auto_key),
        C::DistributeH => ops::distribute(doc!(), true, auto_key),
        C::DistributeV => ops::distribute(doc!(), false, auto_key),
        C::MatchWidth => ops::match_size(doc!(), true, auto_key),
        C::MatchHeight => ops::match_size(doc!(), false, auto_key),
        C::ToggleAlignToStage => core.settings.tools.align_to_stage = !align_stage,
        C::TextBold | C::TextItalic | C::TextAlignLeft | C::TextAlignCenter | C::TextAlignRight => {
            ops::modify_selection(doc!(), Some(c.label()), auto_key, |e| {
                if let animara_document::ElementKind::Text(t) = &mut e.kind {
                    match c {
                        C::TextBold => {
                            let b = !t.style().bold;
                            t.set_style_all(|s| s.bold = b);
                        }
                        C::TextItalic => {
                            let i = !t.style().italic;
                            t.set_style_all(|s| s.italic = i);
                        }
                        C::TextAlignLeft => t.align = animara_document::TextAlign::Left,
                        C::TextAlignCenter => t.align = animara_document::TextAlign::Center,
                        _ => t.align = animara_document::TextAlign::Right,
                    }
                }
            });
        }
        C::TextToOutlines => {
            let st = doc!();
            let texts: Vec<_> = st.selected().into_iter().filter(|(_, e)| matches!(e.kind, animara_document::ElementKind::Text(_))).map(|(_, e)| e.id).collect();
            for id in texts {
                st.selection = vec![id];
                ops::break_apart(st, auto_key);
                ops::break_apart(st, auto_key);
            }
        }
        C::RunScriptCommand => {
            if let Some(p) = rfd::FileDialog::new().add_filter("Animara command (Rhai)", &["rhai"]).pick_file() {
                match std::fs::read_to_string(&p) {
                    Ok(source) => {
                        let plugin = animara_scripting::Plugin { name: p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), description: String::new(), path: p, source };
                        run_plugin(core, &plugin);
                    }
                    Err(e) => core.error(format!("Could not read command: {e}")),
                }
            }
        }
        C::ManagePlugins => core.dialog = Some(Dialog::Plugins),
        C::PlayStop => {
            let mute = core.settings.mute;
            let st = doc!();
            st.playing = !st.playing;
            st.play_anchor = None;
            if st.playing {
                if st.frame + 1 >= st.timeline().length() {
                    st.frame = 0;
                }
                st.selection.clear();
                if !mute {
                    let tl = st.timeline().clone();
                    let doc = st.doc.clone();
                    let f = st.frame;
                    core.audio.play_timeline(&doc, &tl, f);
                }
            } else {
                core.audio.stop();
            }
        }
        C::Rewind => {
            let st = doc!();
            st.frame = 0;
            st.selection.clear();
        }
        C::GoToEnd => {
            let st = doc!();
            st.frame = st.timeline().length().saturating_sub(1);
            st.selection.clear();
        }
        C::StepForward => {
            let st = doc!();
            st.frame += 1;
            st.playing = false;
            st.selection.clear();
        }
        C::StepBackward => {
            let st = doc!();
            st.frame = st.frame.saturating_sub(1);
            st.playing = false;
            st.selection.clear();
        }
        C::NextKeyframe | C::PrevKeyframe => {
            let st = doc!();
            if let Some(l) = st.current_layer() {
                let f = if c == C::NextKeyframe { l.next_keyframe_after(st.frame) } else { l.prev_keyframe_before(st.frame) };
                if let Some(f) = f {
                    st.frame = f;
                    st.selection.clear();
                }
            }
        }
        C::TestMovie | C::DebugMovie => {
            if let Some(st) = core.doc() {
                if c == C::DebugMovie {
                    let mut errors = 0;
                    let mut lines = Vec::new();
                    let tls: Vec<(String, &animara_document::Timeline)> = st
                        .doc
                        .scenes
                        .iter()
                        .map(|s| (s.name.clone(), &s.timeline))
                        .chain(st.doc.library.symbols.iter().map(|s| (s.name.clone(), &s.timeline)))
                        .collect();
                    for (name, tl) in tls {
                        for l in &tl.layers {
                            for k in &l.frames {
                                if let Err(e) = animara_scripting::check_syntax(&k.script) {
                                    errors += 1;
                                    lines.push(format!("{name} › {} › frame {}: {e}", l.name, k.start + 1));
                                }
                            }
                        }
                    }
                    for l in lines {
                        core.error(l);
                    }
                    core.info(format!("Script check finished: {errors} error(s)."));
                }
                let st = core.doc().unwrap();
                core.player = Some(crate::player::Player::new(st.doc.clone(), st.scene, core.settings.mute));
                core.audio.stop();
            }
        }
        C::ToggleLoop => {
            let st = doc!();
            st.loop_playback = !st.loop_playback;
        }
        C::ToggleOnionSkin => {
            let st = doc!();
            st.onion.enabled = !st.onion.enabled;
        }
        C::ToggleOnionOutline => {
            let st = doc!();
            st.onion.outline = !st.onion.outline;
        }
        C::MuteSounds => {
            core.settings.mute = !core.settings.mute;
            core.audio.stop();
        }
        C::ValidateDocument => {
            if let Some(st) = core.doc() {
                let problems = st.doc.validate();
                if problems.is_empty() {
                    core.info("Document is valid.");
                }
                for p in problems {
                    core.error(p);
                }
            }
        }
        C::ShowTimeline => return Some(UiAction::ShowPanel(Tab::Timeline)),
        C::ShowProperties => return Some(UiAction::ShowPanel(Tab::Properties)),
        C::ShowLibrary => return Some(UiAction::ShowPanel(Tab::Library)),
        C::ShowColor => return Some(UiAction::ShowPanel(Tab::Color)),
        C::ShowSwatches => return Some(UiAction::ShowPanel(Tab::Swatches)),
        C::ShowAlign => return Some(UiAction::ShowPanel(Tab::Align)),
        C::ShowTransform => return Some(UiAction::ShowPanel(Tab::Transform)),
        C::ShowHistory => return Some(UiAction::ShowPanel(Tab::History)),
        C::ShowActions => return Some(UiAction::ShowPanel(Tab::Actions)),
        C::ShowOutput => return Some(UiAction::ShowPanel(Tab::Output)),
        C::ShowMotionEditor => return Some(UiAction::ShowPanel(Tab::MotionEditor)),
        C::ShowScenes => return Some(UiAction::ShowPanel(Tab::Scenes)),
        C::ShowInfo => return Some(UiAction::ShowPanel(Tab::Info)),
        C::ShowComponents => return Some(UiAction::ShowPanel(Tab::Components)),
        C::ShowAssets => return Some(UiAction::ShowPanel(Tab::Assets)),
        C::ResetWorkspace => return Some(UiAction::ResetWorkspace),
        C::SaveWorkspace => core.dialog = Some(Dialog::SaveWorkspace { name: "My Workspace".into() }),
        C::ToggleTheme => {
            core.settings.dark = !core.settings.dark;
            crate::theme::apply(ctx, &core.settings);
            for d in &mut core.docs {
                d.tex_key = 0;
            }
        }
        C::HidePanels => return Some(UiAction::TogglePanels),
        C::About => core.dialog = Some(Dialog::About),
        C::ShowHelp => core.dialog = Some(Dialog::Help),
    }
    None
}

pub fn close_doc(core: &mut AppCore, uid: u64) {
    if let Some(i) = core.docs.iter().position(|d| d.uid == uid) {
        io_ops::remove_recovery(uid);
        core.docs.remove(i);
        if core.active >= core.docs.len() {
            core.active = core.docs.len().saturating_sub(1);
        }
    }
}
