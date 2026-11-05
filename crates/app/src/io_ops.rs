//! File operations: new/open/save, import, export, publish, autosave and crash recovery.

use crate::core::{AppCore, ExportKind, ExportSettings, Settings};
use crate::state::DocState;
use animara_document::{Document, Element, ElementKind, Keyframe, SoundRef};
use animara_exporters::{ExportOptions, VideoFormat};
use std::path::{Path, PathBuf};

pub const NATIVE_EXT: &str = "anim";

pub fn new_document(core: &mut AppCore, width: f64, height: f64, fps: f64) {
    let mut doc = Document::new(width, height, fps);
    let n = (1..).find(|i| !core.docs.iter().any(|d| d.doc.name == format!("Untitled-{i}"))).unwrap_or(1);
    doc.name = format!("Untitled-{n}");
    core.docs.push(DocState::new(doc, None));
    core.active = core.docs.len() - 1;
}

fn add_doc(core: &mut AppCore, doc: Document, path: Option<PathBuf>) {
    core.docs.push(DocState::new(doc, path));
    core.active = core.docs.len() - 1;
}

/// Open any supported file as a document.
pub fn open_path(core: &mut AppCore, path: &Path) {
    if let Some(i) = core.docs.iter().position(|d| d.path.as_deref() == Some(path)) {
        core.active = i;
        return;
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let result: Result<(Document, Vec<String>, bool), String> = match ext.as_str() {
        NATIVE_EXT => animara_document::load_document(path).map(|d| (d, Vec::new(), true)).map_err(|e| e.to_string()),
        "xfl" => animara_importers::import_xfl(path).map(|r| (r.document, r.warnings, false)).map_err(|e| e.to_string()),
        "fla" => animara_importers::import_fla(path).map(|r| (r.document, r.warnings, false)).map_err(|e| e.to_string()),
        "svg" => animara_importers::import_svg(path)
            .map(|r| {
                let mut d = Document::new(r.width.max(1.0), r.height.max(1.0), 24.0);
                d.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                d.library.bitmaps = r.bitmaps;
                d.scenes[0].timeline.layers[0].frames[0].elements = r.elements;
                (d, r.warnings, false)
            })
            .map_err(|e| e.to_string()),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => animara_importers::import_image(path)
            .map(|b| {
                let mut d = Document::new(b.width as f64, b.height as f64, 24.0);
                d.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let e = Element::new(ElementKind::Bitmap(animara_document::BitmapInstance { asset: b.id }));
                d.library.bitmaps.push(b);
                d.scenes[0].timeline.layers[0].frames[0].elements.push(e);
                (d, Vec::new(), false)
            })
            .map_err(|e| e.to_string()),
        _ => Err(format!("unsupported file type .{ext}")),
    };
    match result {
        Ok((doc, warnings, native)) => {
            let n = warnings.len();
            for w in warnings {
                core.warn(format!("{}: {w}", path.display()));
            }
            add_doc(core, doc, native.then(|| path.to_path_buf()));
            core.settings.add_recent(path.to_path_buf());
            core.info(format!(
                "Opened {}{}",
                path.display(),
                if n > 0 { format!(" ({n} compatibility warnings — see Output)") } else { String::new() }
            ));
        }
        Err(e) => core.error(format!("Could not open {}: {e}", path.display())),
    }
}

pub fn open_dialog(core: &mut AppCore) {
    let picked = rfd::FileDialog::new()
        .add_filter("All supported", &[NATIVE_EXT, "fla", "xfl", "svg", "png", "jpg", "jpeg", "gif", "webp"])
        .add_filter("Animara document", &[NATIVE_EXT])
        .add_filter("Animate document (FLA/XFL)", &["fla", "xfl"])
        .add_filter("SVG", &["svg"])
        .pick_files();
    for p in picked.unwrap_or_default() {
        open_path(core, &p);
    }
}

pub fn save(core: &mut AppCore, save_as: bool) -> bool {
    let Some(st) = core.doc_mut() else { return false };
    let path = match (&st.path, save_as) {
        (Some(p), false) => Some(p.clone()),
        _ => rfd::FileDialog::new()
            .set_file_name(format!("{}.{NATIVE_EXT}", st.doc.name))
            .add_filter("Animara document", &[NATIVE_EXT])
            .save_file(),
    };
    let Some(mut path) = path else { return false };
    if path.extension().is_none() {
        path.set_extension(NATIVE_EXT);
    }
    let st = core.doc_mut().unwrap();
    if let Some(stem) = path.file_stem() {
        st.doc.name = stem.to_string_lossy().into_owned();
    }
    match animara_document::save_document(&st.doc, &path) {
        Ok(()) => {
            st.path = Some(path.clone());
            st.dirty = false;
            let uid = st.uid;
            remove_recovery(uid);
            core.settings.add_recent(path.clone());
            core.info(format!("Saved {}", path.display()));
            true
        }
        Err(e) => {
            core.error(format!("Save failed: {e}"));
            false
        }
    }
}

/// Import files to the stage (current keyframe) or only to the library.
pub fn import(core: &mut AppCore, to_stage: bool) {
    let files = rfd::FileDialog::new()
        .add_filter(
            "All importable",
            &[
                "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "wav", "mp3", "ogg", "flac", "aac", "m4a", "mp4", "mov", "webm", "mkv",
                "avi", "fla", "xfl", "anim",
            ],
        )
        .add_filter("Images", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
        .add_filter("SVG", &["svg"])
        .add_filter("Audio", &["wav", "mp3", "ogg", "flac", "aac", "m4a"])
        .add_filter("Video", &["mp4", "mov", "webm", "mkv", "avi"])
        .pick_files()
        .unwrap_or_default();
    for f in files {
        import_path(core, &f, to_stage);
    }
}

pub fn import_path(core: &mut AppCore, path: &Path, to_stage: bool) {
    if core.docs.is_empty() {
        new_document(core, 1280.0, 720.0, 30.0);
    }
    let auto_key = core.settings.tools.auto_keyframe;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" => {
            if ext == "gif"
                && to_stage
                && let Ok(frames) = animara_importers::import_gif_frames(path)
                && frames.len() > 1
            {
                // Animated GIF → one keyframe per frame.
                let st = core.doc_mut().unwrap();
                st.checkpoint("Import Animated GIF");
                let li = st.layer;
                let start = st.frame;
                let mut f = start;
                for b in frames {
                    let id = b.id;
                    st.doc.library.bitmaps.push(b);
                    let layer = &mut st.timeline_mut().layers[li];
                    layer.insert_blank_keyframe(f);
                    if let Some(k) = layer.keyframe_at_mut(f) {
                        k.elements.push(Element::new(ElementKind::Bitmap(animara_document::BitmapInstance { asset: id })));
                    }
                    f += 1;
                }
                core.info(format!("Imported {} frames from {name}", f - start));
                return;
            }
            match animara_importers::import_image(path) {
                Ok(b) => {
                    let id = b.id;
                    let st = core.doc_mut().unwrap();
                    st.checkpoint("Import Image");
                    st.doc.library.bitmaps.push(b);
                    if to_stage {
                        let e = Element::new(ElementKind::Bitmap(animara_document::BitmapInstance { asset: id }));
                        if let Some(k) = st.editable_keyframe(auto_key) {
                            k.elements.push(e);
                        }
                    }
                    core.info(format!("Imported {name}"));
                }
                Err(e) => core.error(format!("Import {name} failed: {e}")),
            }
        }
        "svg" => match animara_importers::import_svg(path) {
            Ok(r) => {
                let st = core.doc_mut().unwrap();
                st.checkpoint("Import SVG");
                st.doc.library.bitmaps.extend(r.bitmaps);
                // SVG artwork becomes a graphic symbol; place it on stage if requested.
                let mut sym = animara_document::Symbol::new(&st.doc.library.unique_name(&name), animara_document::SymbolKind::Graphic);
                sym.timeline.layers[0].frames[0].elements = r.elements;
                sym.folder = "Imported".into();
                let sid = st.doc.library.add_symbol(sym);
                if to_stage {
                    let e = st.doc.instantiate(sid, 0.0, 0.0);
                    if let Some(k) = st.editable_keyframe(auto_key) {
                        k.elements.push(e);
                    }
                }
                for w in r.warnings {
                    core.warn(format!("{name}: {w}"));
                }
                core.info(format!("Imported {name}"));
            }
            Err(e) => core.error(format!("Import {name} failed: {e}")),
        },
        "wav" | "mp3" | "ogg" | "flac" | "aac" | "m4a" => match animara_media::import_sound(path) {
            Ok(s) => {
                let id = s.id;
                let dur = s.duration;
                let st = core.doc_mut().unwrap();
                st.checkpoint("Import Sound");
                st.doc.library.sounds.push(s);
                if to_stage {
                    if let Some(k) = st.editable_keyframe(auto_key) {
                        k.sound = Some(SoundRef::new(id));
                    }
                    let li = st.layer;
                    let f = st.frame + (dur * st.doc.fps).ceil() as u32;
                    st.timeline_mut().layers[li].extend_to(f.saturating_sub(1));
                }
                core.info(format!("Imported {name} ({dur:.2}s)"));
            }
            Err(e) => core.error(format!("Import {name} failed: {e}")),
        },
        "mp4" | "mov" | "webm" | "mkv" | "avi" => {
            let fps = core.doc().map(|d| d.doc.fps).unwrap_or(24.0);
            match animara_media::import_video(path, fps, 3000, 1920) {
                Ok(v) => {
                    let st = core.doc_mut().unwrap();
                    st.checkpoint("Import Video");
                    // Embedded video: a new layer with one bitmap keyframe per video frame.
                    let n = st.timeline().layers.len();
                    let at = st.layer.min(n);
                    let lid = st.timeline_mut().add_layer(&format!("Video: {name}"), at);
                    let mut frames = Vec::new();
                    for (i, png) in v.frames.into_iter().enumerate() {
                        if let Ok(b) = animara_importers::import_image_bytes(&format!("{name} frame {:05}", i + 1), png) {
                            let mut b = b;
                            b.folder = format!("Video/{name}");
                            frames.push(b.id);
                            st.doc.library.bitmaps.push(b);
                        }
                    }
                    let start = st.frame;
                    let layer = st.timeline_mut().layer_mut(lid).unwrap();
                    layer.frames = vec![Keyframe::new(0, start.max(1))];
                    if start == 0 {
                        layer.frames.clear();
                    }
                    for (i, id) in frames.iter().enumerate() {
                        let mut k = Keyframe::new(start + i as u32, 1);
                        k.elements.push(Element::new(ElementKind::Bitmap(animara_document::BitmapInstance { asset: *id })));
                        layer.frames.push(k);
                    }
                    if let Some(wav) = v.audio_wav
                        && let Ok(s) = animara_media::sound_from_wav_bytes(&format!("{name} audio"), wav)
                    {
                        let sid = s.id;
                        st.doc.library.sounds.push(s);
                        if let Some(k) = st.timeline_mut().layer_mut(lid).unwrap().keyframe_at_mut(start) {
                            k.sound = Some(SoundRef::new(sid));
                        }
                    }
                    core.info(format!("Imported {} video frames from {name} ({}×{})", frames.len(), v.width, v.height));
                }
                Err(e) => core.error(format!("Video import failed: {e}")),
            }
        }
        "fla" | "xfl" | NATIVE_EXT => open_external_library(core, path),
        _ => core.error(format!("Unsupported import type: {name}")),
    }
}

/// Merge another document's library into the current one.
pub fn open_external_library(core: &mut AppCore, path: &Path) {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let src = match ext.as_str() {
        "fla" => animara_importers::import_fla(path).map(|r| r.document).map_err(|e| e.to_string()),
        "xfl" => animara_importers::import_xfl(path).map(|r| r.document).map_err(|e| e.to_string()),
        _ => animara_document::load_document(path).map_err(|e| e.to_string()),
    };
    match src {
        Ok(src) => {
            if core.docs.is_empty() {
                new_document(core, src.width, src.height, src.fps);
            }
            let st = core.doc_mut().unwrap();
            st.checkpoint("Import Library");
            let ids = crate::thumbs::merge_library(&mut st.doc, &src);
            core.info(format!("Imported {} symbols from {}", ids.len(), path.display()));
        }
        Err(e) => core.error(format!("Could not read library {}: {e}", path.display())),
    }
}

pub fn default_export(st: &DocState, kind: ExportKind) -> ExportSettings {
    let total = st.doc.total_frames();
    ExportSettings {
        kind,
        scale: st.doc.publish.scale,
        fps: st.doc.fps,
        first: 1,
        last: total,
        transparent: st.doc.publish.transparent,
        quality: st.doc.publish.quality,
        loop_count: if st.doc.publish.loop_playback { 0 } else { 1 },
        include_audio: st.doc.publish.include_audio,
    }
}

/// Run an export; asks for the output path.
pub fn run_export(core: &mut AppCore, s: &ExportSettings) {
    let Some(st) = core.doc() else { return };
    let name = st.doc.name.clone();
    let path = match s.kind {
        ExportKind::PngSequence | ExportKind::SvgSequence => rfd::FileDialog::new().set_title("Choose an output folder").pick_folder(),
        _ => rfd::FileDialog::new()
            .set_file_name(format!("{name}.{}", s.kind.extension()))
            .add_filter(s.kind.name(), &[s.kind.extension()])
            .save_file(),
    };
    let Some(path) = path else { return };
    export_to(core, s, &path);
}

pub fn export_to(core: &mut AppCore, s: &ExportSettings, path: &Path) {
    let Some(st) = core.doc() else { return };
    let doc = st.doc.clone();
    let global_frame = {
        let before: u32 = doc.scenes[..st.scene.min(doc.scenes.len())].iter().map(|sc| sc.timeline.length()).sum();
        before + st.frame
    };
    let mut doc = doc;
    doc.fps = s.fps;
    let opts = ExportOptions {
        scale: s.scale,
        range: Some((s.first.saturating_sub(1), s.last.saturating_sub(1))),
        transparent: s.transparent,
        quality: s.quality,
        loop_count: s.loop_count,
        include_audio: s.include_audio,
    };
    let base = doc.name.clone();
    let t0 = std::time::Instant::now();
    let result: Result<Vec<String>, String> = {
        let e = |e: animara_exporters::ExportError| e.to_string();
        match s.kind {
            ExportKind::Png => animara_exporters::export_png(&doc, global_frame, path, &opts).map(|_| vec![]).map_err(e),
            ExportKind::PngSequence => {
                animara_exporters::export_png_sequence(&doc, path, &base, &opts).map(|v| vec![format!("{} files", v.len())]).map_err(e)
            }
            ExportKind::Gif => animara_exporters::export_gif(&doc, path, &opts).map(|_| vec![]).map_err(e),
            ExportKind::Apng => animara_exporters::export_apng(&doc, path, &opts).map(|_| vec![]).map_err(e),
            ExportKind::WebP => animara_exporters::export_video(&doc, path, VideoFormat::WebP, &opts).map(|_| vec![]).map_err(e),
            ExportKind::Mp4 => animara_exporters::export_video(&doc, path, VideoFormat::Mp4, &opts).map(|_| vec![]).map_err(e),
            ExportKind::WebM => animara_exporters::export_video(&doc, path, VideoFormat::WebM, &opts).map(|_| vec![]).map_err(e),
            ExportKind::Mov => animara_exporters::export_video(&doc, path, VideoFormat::Mov, &opts).map(|_| vec![]).map_err(e),
            ExportKind::Svg => animara_exporters::export_svg_frame(&doc, global_frame, path, &opts).map(|_| vec![]).map_err(e),
            ExportKind::SvgSequence => {
                animara_exporters::export_svg_sequence(&doc, path, &base, &opts).map(|v| vec![format!("{} files", v.len())]).map_err(e)
            }
            ExportKind::SpriteSheet => {
                animara_exporters::export_sprite_sheet(&doc, path, &opts).map(|j| vec![format!("metadata: {}", j.display())]).map_err(e)
            }
            ExportKind::Lottie => animara_exporters::export_lottie(&doc, path, &opts).map_err(e),
            ExportKind::Html5 => animara_exporters::export_html5(&doc, path, &opts).map(|_| vec![]).map_err(e),
            ExportKind::Swf => animara_exporters::export_swf(&doc, path, &opts).map_err(e),
            ExportKind::Xfl => {
                let p = if path.extension().is_some_and(|x| x == "fla") {
                    animara_exporters::export_fla(&doc, path).map(|_| path.to_path_buf())
                } else {
                    animara_exporters::export_xfl(&doc, path)
                };
                p.map(|m| vec![format!("written {}", m.display())]).map_err(e)
            }
        }
    };
    match result {
        Ok(notes) => {
            for n in notes {
                core.warn(format!("{}: {n}", s.kind.name()));
            }
            core.info(format!("Exported {} → {} in {:.1}s", s.kind.name(), path.display(), t0.elapsed().as_secs_f64()));
        }
        Err(e) => core.error(format!("Export failed: {e}")),
    }
}

/// Publish: HTML5 Canvas page + animated GIF poster next to the document (or a chosen folder).
pub fn publish(core: &mut AppCore) {
    let Some(st) = core.doc() else { return };
    let dir = match st.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        Some(d) => d,
        None => match rfd::FileDialog::new().set_title("Publish to folder").pick_folder() {
            Some(d) => d,
            None => return,
        },
    };
    let out = if st.doc.publish.output_name.is_empty() || st.doc.publish.output_name == "untitled" {
        st.doc.name.clone()
    } else {
        st.doc.publish.output_name.clone()
    };
    let s = default_export(st, ExportKind::Html5);
    export_to(core, &s, &dir.join(format!("{out}.html")));
    let s = default_export(core.doc().unwrap(), ExportKind::Gif);
    export_to(core, &s, &dir.join(format!("{out}.gif")));
}

// ------------------------------------------------------------------ autosave / recovery

pub fn recovery_dir() -> PathBuf {
    Settings::config_dir().join("recovery")
}

fn recovery_path(uid: u64) -> PathBuf {
    recovery_dir().join(format!("{}-{uid}.{NATIVE_EXT}", std::process::id()))
}

pub fn remove_recovery(uid: u64) {
    let _ = std::fs::remove_file(recovery_path(uid));
}

/// Write recovery snapshots of modified documents.
pub fn autosave(core: &mut AppCore) {
    let _ = std::fs::create_dir_all(recovery_dir());
    for st in &mut core.docs {
        if st.dirty && st.revision != st.last_autosave_revision && animara_document::save_document(&st.doc, &recovery_path(st.uid)).is_ok()
        {
            st.last_autosave_revision = st.revision;
        }
    }
}

/// Recovery files left by a previous session that did not exit cleanly.
pub fn pending_recovery() -> Vec<PathBuf> {
    let me = format!("{}-", std::process::id());
    std::fs::read_dir(recovery_dir())
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| !n.to_string_lossy().starts_with(&me))).collect())
        .unwrap_or_default()
}

pub fn clean_shutdown(core: &AppCore) {
    for st in &core.docs {
        remove_recovery(st.uid);
    }
}
