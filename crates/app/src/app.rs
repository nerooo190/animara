//! The application window: menu bar, tool bar, docked panels, status bar.

use crate::commands::Command as C;
use crate::core::{AppCore, Dialog};
use crate::icons::{self, Icon};
use crate::menus::{self, UiAction};
use crate::panels::{self, PropTab};
use crate::timeline_ui::{TimelineUiState, timeline_ui};
use crate::tools::Tool;
use egui_stack::{DockArea, DockState, NodeIndex, Style, TabViewer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Stage,
    Timeline,
    Output,
    Properties,
    Library,
    Assets,
    Color,
    Swatches,
    Align,
    Transform,
    History,
    Actions,
    MotionEditor,
    Scenes,
    Info,
    Components,
}

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Stage => "Stage",
            Tab::Timeline => "Timeline",
            Tab::Output => "Output",
            Tab::Properties => "Properties",
            Tab::Library => "Library",
            Tab::Assets => "Assets",
            Tab::Color => "Color",
            Tab::Swatches => "Swatches",
            Tab::Align => "Align",
            Tab::Transform => "Transform",
            Tab::History => "History",
            Tab::Actions => "Actions",
            Tab::MotionEditor => "Motion Editor",
            Tab::Scenes => "Scene",
            Tab::Info => "Info",
            Tab::Components => "Components",
        }
    }
}

/// Default "Essentials"-style layout: stage centre, timeline below, panel stack right.
pub fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Stage]);
    let surface = dock.main_surface_mut();
    let [stage, _right] =
        surface.split_right(NodeIndex::root(), 0.74, vec![Tab::Properties, Tab::Library, Tab::Assets, Tab::Color, Tab::Align]);
    let [_stage, _bottom] = surface.split_below(stage, 0.66, vec![Tab::Timeline, Tab::Output, Tab::MotionEditor, Tab::Actions]);
    let [_r, _lower] =
        surface.split_below(_right, 0.72, vec![Tab::Swatches, Tab::Transform, Tab::History, Tab::Info, Tab::Scenes, Tab::Components]);
    dock
}

struct Viewer<'a> {
    core: &'a mut AppCore,
    tl: &'a mut TimelineUiState,
    prop_tab: &'a mut PropTab,
}

impl TabViewer for Viewer<'_> {
    type Tab = Tab;
    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::unique(*tab)
    }
    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        match tab {
            Tab::Stage => {
                document_tabs(self.core, ui);
                crate::stage::stage_ui(self.core, ui);
                // Library drag & drop onto the stage.
                if let Some(payload) = egui::DragAndDrop::payload::<panels::LibDrag>(ui.ctx())
                    && ui.input(|i| i.pointer.any_released())
                    && ui.ui_contains_pointer()
                {
                    let id = payload.0;
                    egui::DragAndDrop::clear_payload(ui.ctx());
                    let p = self.core.tstate.last_pointer_doc;
                    let auto_key = self.core.settings.tools.auto_keyframe;
                    panels::place_library_item(self.core, id, Some(p), auto_key);
                }
            }
            Tab::Timeline => timeline_ui(self.core, ui, self.tl),
            Tab::Output => panels::output_panel(self.core, ui),
            Tab::Properties => panels::properties(self.core, ui, self.prop_tab),
            Tab::Library => panels::library(self.core, ui),
            Tab::Assets => panels::assets_panel(self.core, ui),
            Tab::Color => panels::color_panel(self.core, ui),
            Tab::Swatches => panels::swatches(self.core, ui),
            Tab::Align => panels::align_panel(self.core, ui),
            Tab::Transform => panels::transform_panel(self.core, ui),
            Tab::History => panels::history_panel(self.core, ui),
            Tab::Actions => panels::actions_panel(self.core, ui),
            Tab::MotionEditor => panels::motion_editor(self.core, ui),
            Tab::Scenes => panels::scenes_panel(self.core, ui),
            Tab::Info => panels::info_panel(self.core, ui),
            Tab::Components => panels::components_panel(self.core, ui),
        }
    }
    fn is_closeable(&self, tab: &Tab) -> bool {
        *tab != Tab::Stage
    }
    fn allowed_in_windows(&self, _tab: &mut Tab) -> bool {
        true
    }
    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

fn document_tabs(core: &mut AppCore, ui: &mut egui::Ui) {
    let mut close = None;
    ui.horizontal(|ui| {
        for (i, d) in core.docs.iter().enumerate() {
            let r = ui.selectable_label(i == core.active, d.title());
            if r.clicked() {
                core.active = i;
            }
            if r.middle_clicked() {
                close = Some(d.uid);
            }
            r.on_hover_text(d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not saved".into()));
            if ui.small_button("✕").on_hover_text("Close document").clicked() {
                close = Some(d.uid);
            }
            ui.separator();
        }
        if ui.small_button("+").on_hover_text("New document").clicked() {
            core.run(C::NewDocument);
        }
    });
    if let Some(uid) = close {
        if core.docs.iter().any(|d| d.uid == uid && d.dirty) {
            core.dialog = Some(Dialog::ConfirmClose { uid, then_exit: false });
        } else {
            menus::close_doc(core, uid);
        }
    }
}

fn toolbar(core: &mut AppCore, ui: &mut egui::Ui) {
    ui.add_space(4.0);
    ui.vertical_centered(|ui| {
        for t in Tool::TOOLBAR {
            match t {
                Some(t) => {
                    if icons::button(ui, Icon::Tool(*t), core.tool == *t, t.name(), 30.0).clicked() {
                        if core.tool == Tool::Pen && *t != Tool::Pen {
                            crate::stage::finish_pen(core, false);
                        }
                        core.tool = *t;
                    }
                }
                None => {
                    ui.add_space(2.0);
                    ui.separator();
                }
            }
        }
        ui.separator();
        // Stroke / fill color chips.
        let s = &mut core.settings.tools;
        ui.label(egui::RichText::new("✏").small());
        let mut sc = crate::stage::c32(s.stroke_color);
        if ui.color_edit_button_srgba(&mut sc).on_hover_text("Stroke color").changed() {
            s.stroke_color = panels::to_color(sc);
        }
        ui.label(egui::RichText::new("⬛").small());
        let mut fc = crate::stage::c32(s.fill_color);
        if ui.color_edit_button_srgba(&mut fc).on_hover_text("Fill color").changed() {
            s.fill_color = panels::to_color(fc);
        }
        if ui.small_button("⇄").on_hover_text("Swap colors").clicked() {
            std::mem::swap(&mut s.stroke_color, &mut s.fill_color);
        }
    });
}

pub struct AnimaraApp {
    pub core: AppCore,
    dock: DockState<Tab>,
    tl: TimelineUiState,
    prop_tab: PropTab,
    saved_dock: Option<DockState<Tab>>,
    closing: bool,
}

impl AnimaraApp {
    pub fn new(core: AppCore) -> AnimaraApp {
        let dock = core
            .settings
            .workspaces
            .get(&core.settings.current_workspace)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(default_layout);
        AnimaraApp { core, dock, tl: TimelineUiState::default(), prop_tab: PropTab::Tool, saved_dock: None, closing: false }
    }

    fn show_panel(&mut self, tab: Tab) {
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
        } else {
            self.dock.push_to_focused_leaf(tab);
        }
    }

    fn playback(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(st) = self.core.doc_mut() else { return };
        if !st.playing {
            return;
        }
        let len = st.timeline().length();
        let (t0, f0) = *st.play_anchor.get_or_insert((now, st.frame));
        let elapsed = now - t0;
        let target = f0 as f64 + elapsed * st.doc.fps;
        let mut f = target.floor() as u32;
        if f >= len {
            if st.loop_playback {
                f %= len.max(1);
                st.play_anchor = Some((now, f));
                let doc = st.doc.clone();
                let tl = st.timeline().clone();
                if !self.core.settings.mute {
                    self.core.audio.play_timeline(&doc, &tl, f);
                }
            } else {
                f = len.saturating_sub(1);
                st.playing = false;
            }
        }
        let st = self.core.doc_mut().unwrap();
        st.frame = f;
        ctx.request_repaint();
    }
}

impl eframe::App for AnimaraApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Dropped files: open documents, import everything else.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if matches!(ext.as_str(), "anim" | "fla" | "xfl") || self.core.docs.is_empty() {
                crate::io_ops::open_path(&mut self.core, &p);
            } else {
                crate::io_ops::import_path(&mut self.core, &p, true);
            }
        }
        self.playback(&ctx);
        if !ctx.egui_wants_keyboard_input() || self.core.doc().is_some_and(|d| d.text_edit.is_some()) {
            crate::stage::stage_keys(&mut self.core, &ctx);
        }
        menus::handle_shortcuts(&mut self.core, &ctx);

        egui::Panel::top("menubar").show(ui, |ui| {
            ui.horizontal(|ui| {
                menus::menu_bar(&mut self.core, ui);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("▶ Test").on_hover_text("Test Movie (Ctrl+Enter)").clicked() {
                        self.core.run(C::TestMovie);
                    }
                    if ui.button("⇪ Publish").clicked() {
                        self.core.run(C::Publish);
                    }
                    let names: Vec<String> = std::iter::once("Essentials".to_string())
                        .chain(self.core.settings.workspaces.keys().filter(|k| *k != "Essentials").cloned())
                        .collect();
                    egui::ComboBox::from_id_salt("workspace").selected_text(&self.core.settings.current_workspace).show_ui(ui, |ui| {
                        for n in names {
                            if ui.selectable_label(self.core.settings.current_workspace == n, &n).clicked() {
                                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("load_workspace"), n.clone()));
                            }
                        }
                    });
                });
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.core.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(st) = self.core.doc() {
                        let p = self.core.tstate.last_pointer_doc;
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.0}%  ·  x {:.0}  y {:.0}  ·  frame {}/{}  ·  {}",
                                st.view.zoom * 100.0,
                                p.x,
                                p.y,
                                st.frame + 1,
                                st.timeline().length(),
                                self.core.tool.short()
                            ))
                            .small(),
                        );
                    }
                });
            });
        });
        if !self.core.hide_panels {
            egui::Panel::left("tools").exact_size(48.0).resizable(false).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| toolbar(&mut self.core, ui));
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let mut viewer = Viewer { core: &mut self.core, tl: &mut self.tl, prop_tab: &mut self.prop_tab };
            let mut style = Style::from_egui(ui.style().as_ref());
            style.tab_bar.height = 24.0;
            DockArea::new(&mut self.dock).style(style).show_leaf_collapse_buttons(false).show_inside(ui, &mut viewer);
        });

        crate::dialogs::dialogs(&mut self.core, &ctx);

        // Test Movie window.
        if let Some(p) = &mut self.core.player {
            let now = ctx.input(|i| i.time);
            p.tick(now);
            let mut open = p.open;
            let title = format!("Test Movie — {}", p.doc.name);
            egui::Window::new(title)
                .open(&mut open)
                .default_size([p.doc.width.min(960.0) as f32, p.doc.height.min(600.0) as f32 + 30.0])
                .resizable(true)
                .show(&ctx, |ui| p.ui(ui));
            for l in p.log.drain(..) {
                self.core.log.push((crate::core::LogLevel::Info, l));
            }
            let p = self.core.player.as_mut().unwrap();
            p.open = open;
            if !open {
                p.stop_audio();
                self.core.player = None;
            }
            ctx.request_repaint();
        }

        // Workspace load/save requests.
        if let Some(name) = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::unique("load_workspace"))) {
            if name == "Essentials" {
                self.dock = default_layout();
            } else if let Some(d) = self.core.settings.workspaces.get(&name).and_then(|s| serde_json::from_str(s).ok()) {
                self.dock = d;
            }
            self.core.settings.current_workspace = name;
        }
        if let Some(name) = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::unique("save_workspace")))
            && let Ok(s) = serde_json::to_string(&self.dock)
        {
            self.core.settings.workspaces.insert(name.clone(), s);
            self.core.settings.current_workspace = name.clone();
            self.core.info(format!("Saved workspace “{name}”"));
        }

        // Execute queued commands.
        let queue = std::mem::take(&mut self.core.queue);
        for c in queue {
            match menus::execute(&mut self.core, c, &ctx) {
                Some(UiAction::ShowPanel(t)) => {
                    if self.core.hide_panels {
                        self.core.hide_panels = false;
                        if let Some(d) = self.saved_dock.take() {
                            self.dock = d;
                        }
                    }
                    self.show_panel(t);
                }
                Some(UiAction::ResetWorkspace) => {
                    self.dock = default_layout();
                    self.core.settings.current_workspace = "Essentials".into();
                }
                Some(UiAction::TogglePanels) => {
                    if self.core.hide_panels {
                        if let Some(d) = self.saved_dock.take() {
                            self.dock = d;
                        }
                    } else {
                        self.saved_dock = Some(std::mem::replace(&mut self.dock, DockState::new(vec![Tab::Stage])));
                    }
                    self.core.hide_panels = !self.core.hide_panels;
                }
                None => {}
            }
        }

        // Autosave for crash recovery.
        let now = ctx.input(|i| i.time);
        if self.core.settings.autosave_secs > 0 && now - self.core.last_autosave > self.core.settings.autosave_secs as f64 {
            self.core.last_autosave = now;
            crate::io_ops::autosave(&mut self.core);
        }

        // Window title.
        let title = match self.core.doc() {
            Some(d) => format!("{} — Animara", d.title()),
            None => "Animara".into(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Close / exit handling with unsaved-changes prompts.
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested || self.core.exit_requested {
            if let Some(d) = self.core.docs.iter().find(|d| d.dirty) {
                if close_requested {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                if !matches!(self.core.dialog, Some(Dialog::ConfirmClose { .. })) {
                    self.core.dialog = Some(Dialog::ConfirmClose { uid: d.uid, then_exit: true });
                }
                self.core.exit_requested = false;
            } else if !self.closing {
                self.closing = true;
                if !close_requested {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if let Ok(s) = serde_json::to_string(&self.dock)
            && self.core.settings.current_workspace != "Essentials"
        {
            let name = self.core.settings.current_workspace.clone();
            self.core.settings.workspaces.insert(name, s);
        }
        self.core.settings.save();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.core.audio.stop();
        crate::io_ops::clean_shutdown(&self.core);
        self.core.settings.save();
    }
}
