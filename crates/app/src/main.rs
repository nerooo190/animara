//! Animara — a 2D vector animation studio.

mod app;
mod audio;
mod commands;
mod core;
mod dialogs;
mod icons;
mod io_ops;
mod menus;
mod ops;
mod panels;
mod player;
mod samples;
mod stage;
mod state;
mod theme;
mod thumbs;
mod timeline_ui;
mod tools;

use crate::core::{AppCore, Dialog, Settings};

/// Procedurally drawn application icon (original artwork).
fn app_icon() -> egui::IconData {
    let n = 64u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 / n as f32, y as f32 / n as f32);
            let i = ((y * n + x) * 4) as usize;
            // Rounded square background.
            let (dx, dy) = ((fx - 0.5).abs() - 0.38, (fy - 0.5).abs() - 0.38);
            let outside = dx.max(0.0).hypot(dy.max(0.0)) - 0.1;
            if outside > 0.0 {
                continue;
            }
            let mut c = [0x1b, 0x2a, 0x4a, 255];
            // Stylised "A" made of two strokes and a bar, in the accent color.
            let left = ((fx - 0.5) + (fy - 0.22) * 0.42).abs() < 0.06 && fy > 0.2 && fy < 0.8;
            let right = ((fx - 0.5) - (fy - 0.22) * 0.42).abs() < 0.06 && fy > 0.2 && fy < 0.8;
            let bar = (fy - 0.6).abs() < 0.045 && (fx - 0.5).abs() < 0.18;
            if left || right || bar {
                c = [0x26, 0x80, 0xEB, 255];
            }
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    egui::IconData { rgba, width: n, height: n }
}

/// Add installed symbol/emoji-capable fonts as fallbacks so UI glyphs render on every platform.
fn install_fallback_fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let candidates = [
        "Noto Sans Symbols 2",
        "Noto Sans Symbols",
        "DejaVu Sans",
        "Segoe UI Symbol",
        "Segoe UI Emoji",
        "Apple Symbols",
        "Noto Sans Math",
        "Arial Unicode MS",
    ];
    for name in candidates {
        if let Some((bytes, index)) = animara_text::fonts().family_bytes(name) {
            let mut data = egui::FontData::from_owned(bytes);
            data.index = index;
            defs.font_data.insert(name.to_string(), std::sync::Arc::new(data));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                defs.families.entry(fam).or_default().push(name.to_string());
            }
        }
    }
    ctx.set_fonts(defs);
}

fn main() -> eframe::Result {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<std::path::PathBuf> = raw
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with('-') && !(*i > 0 && raw[i - 1] == "--sample"))
        .map(|(_, a)| a.into())
        .collect();
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("animara {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let settings = Settings::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Animara")
            .with_app_id("io.github.animara.Animara")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        persist_window: true,
        ..Default::default()
    };
    eframe::run_native(
        "Animara",
        options,
        Box::new(move |cc| {
            let mut core = AppCore::new(settings);
            theme::apply(&cc.egui_ctx, &core.settings);
            install_fallback_fonts(&cc.egui_ctx);
            core.font_families = animara_text::fonts().families();
            // Plugins: create the folder with examples on first run.
            let pdir = Settings::config_dir().join("plugins");
            if !pdir.exists() {
                let _ = std::fs::create_dir_all(&pdir);
                for (name, src) in animara_scripting::EXAMPLE_PLUGINS {
                    let _ = std::fs::write(pdir.join(name), src);
                }
            }
            core.plugins = animara_scripting::load_plugins(&pdir);
            for p in &args {
                io_ops::open_path(&mut core, p);
            }
            // `--sample N` opens a built-in sample document (used for demos and screenshots).
            let argv: Vec<String> = std::env::args().collect();
            if let Some(n) = argv.iter().position(|a| a == "--sample").and_then(|i| argv.get(i + 1)).and_then(|s| s.parse::<usize>().ok())
                && let Some((_, f)) = samples::SAMPLES.get(n)
            {
                core.docs.push(state::DocState::new(f(), None));
                core.active = core.docs.len() - 1;
                core.settings.show_welcome = false;
            }
            let recovery = io_ops::pending_recovery();
            if !recovery.is_empty() {
                core.dialog = Some(Dialog::Recovery { files: recovery });
            } else if core.docs.is_empty() && core.settings.show_welcome && !std::env::args().any(|a| a == "--sample") {
                core.dialog = Some(Dialog::Welcome);
            }
            if core.docs.is_empty() {
                io_ops::new_document(&mut core, 1280.0, 720.0, 30.0);
            }
            if !animara_media::ffmpeg_available() {
                core.warn("FFmpeg not found: video import/export and non-WAV audio import are unavailable.");
            }
            Ok(Box::new(app::AnimaraApp::new(core)))
        }),
    )
}
