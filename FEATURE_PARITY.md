# Feature parity matrix

Statuses: **Not started · Investigating · In progress · Implemented · Verified · Partially compatible · Blocked**.

* **Implemented** — working code reachable from the UI/CLI, covered by automated tests or exercised in the running
  application (screenshots in the session log).
* **Verified** — additionally compared against the reference recording R1 and found to match the observed behaviour.
  Nothing is marked Verified against real Adobe-generated files, because none were available.
* **Partially compatible** — works, with documented gaps.

Evidence IDs: see [`docs/reverse-engineering/evidence-log.md`](docs/reverse-engineering/evidence-log.md).
Test paths are relative to `crates/`.

## Workspace & UI

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Menu bar (File…Help, same order) | Verified | manual (screenshot) | O2 |
| Tool column with grouped tools, colour chips | Implemented | manual | O7 |
| Stage with edit bar, breadcrumb, zoom combo, fit | Implemented | manual | O6, O12 |
| Dockable / tabbable / floating panels | Implemented | manual | O6 |
| Properties panel Tool / Object / Frame / Doc | Verified | manual | O18 |
| Library (folders, search, thumbnails, drag to stage, rename, duplicate, delete, behaviour) | Implemented | manual | O6 |
| Assets panel (Default animated assets / Custom saved assets) | Implemented | manual | O20 |
| Color panel (solid, linear, radial, gradient stop editor, hex, HSB) | Implemented | manual | K |
| Swatches | Implemented | manual | O19 |
| Align / distribute / match size (to stage option) | Implemented | `app ops::tests::align_and_distribute` | O19 |
| Transform panel (scale, rotate, skew, flip, copy & apply) | Implemented | manual | O19 |
| History panel (jump to any state) | Implemented | `document io::tests::undo_redo` | O19 |
| Actions panel (syntax highlighting, snippets, syntax check) | Implemented | `scripting tests` | O19 |
| Motion Editor (ease presets, custom curve, per-property eases) | Implemented | `document ease::tests` | O8 (graph icon) |
| Output panel (trace, warnings, errors) | Implemented | manual | O6 |
| Scene panel (add, duplicate, delete, rename, reorder) | Implemented | manual | O19 |
| Info panel | Implemented | manual | O19 |
| Components panel (button, checkbox, label, progress bar) | Implemented | manual | O19 |
| Document tabs / multi-document editing | Implemented | manual | O19 |
| Context menus (stage, timeline frames, layers, library) | Implemented | manual | — |
| Configurable keyboard shortcuts (capture editor, conflict removal, persisted) | Implemented | `app commands::tests` | O5, O19 |
| Searchable command palette (Ctrl+Shift+P) | Implemented | manual | — (no reference equivalent) |
| Welcome screen, recent documents, sample files | Implemented | manual | O3 |
| New Document dialog with category presets | Verified | manual | O3 |
| Project templates / New from Template browser | Partially compatible | manual | O4 (presets + samples, no separate template browser) |
| Dark and light themes, UI scaling (HiDPI) | Implemented | manual | — |
| Custom workspaces (save, switch, reset) | Implemented | manual | O19 |
| Hide panels (F4) | Implemented | manual | O19 |
| Multiple OS windows / multi-monitor | Partially compatible | — | O19 (floating in-app panels only) |
| Autosave & crash recovery | Implemented | manual | — |
| Preferences | Implemented | manual | — |
| Rulers, guides (drag from ruler), grid, snapping (grid/guides/objects) | Implemented | manual | — |

## Drawing & vector editing

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Selection tool (click, shift-add, marquee, move, alt-drag duplicate, nudge) | Implemented | manual (scripted input) | O11 |
| Subselection (anchors, handles, insert/delete/convert anchor) | Implemented | `geometry path::tests::anchor_editing` | K |
| Free transform (scale, rotate, skew, pivot, shift constraints) | Implemented | manual | O11 |
| Gradient transform (move, rotate/width, height) | Implemented | manual | K |
| Lasso | Implemented | manual | O7 |
| Pen (corner + smooth points, close path) | Implemented | manual | O7 |
| Pencil (straighten / smooth / ink) | Implemented | `geometry smooth::tests` | — |
| Paint brush with pressure & velocity taper (fluid-style) | Implemented | `geometry smooth::tests::brush_outline_has_area` | O7, O14 |
| Tablet pressure | Partially compatible | — | depends on egui touch force events; tilt not available |
| Classic brush | Implemented | manual | O7 |
| Eraser (normal, fills, lines, faucet) | Implemented | manual | O7 |
| Line, rectangle, oval, polystar | Implemented | manual (scripted input) | O7 |
| Rectangle / oval primitives (editable radius, angles, inner radius) | Implemented | manual | K |
| Paint bucket (closed paths, gradients) | Partially compatible | manual | gap-closing fill of regions bounded by separate strokes not implemented |
| Ink bottle, eyedropper | Implemented | manual | O7 |
| Text tool (click / box, inline editing with caret) | Implemented | manual | O7 |
| Hand, zoom (tool, wheel, pinch, presets, fit) | Implemented | manual | O6 |
| Bézier path model, SVG path data | Implemented | `geometry path::tests` | — |
| Boolean union / intersect / punch / crop | Implemented | `geometry boolean::tests`, `app ops::tests::combine_union` | — |
| Smooth / straighten / optimize / lines → fills | Implemented | manual | — |
| Group / ungroup / break apart (instances, text twice → outlines, bitmaps) | Implemented | `app ops::tests::group_ungroup_and_symbol` | — |
| Arrange (front/back/forward/backward), lock objects | Implemented | manual | — |
| Merge-drawing model (shapes merging on contact) | Not started | — | Animara always uses object drawing |
| Variable-width stroke profiles on lines | Not started | — | — |
| Custom brush presets / brush library | Not started | — | O19 |
| Bitmap painting | Not started | — | — |
| Antialiasing | Implemented | `renderer tests` | — |

## Timeline & animation

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Frames, keyframes, blank keyframes, extend, remove, clear | Implemented | `document timeline::tests` | O8, O14 |
| Multi-frame selection, copy/cut/paste frames, reverse, convert to keyframes | Implemented | `document timeline::tests::copy_paste_reverse` | — |
| Drag keyframes to retime | Implemented | `document timeline::tests::move_keyframe_between_neighbours` | — |
| Layers: add, delete, duplicate, rename, reorder (drag), visibility, lock, outline, colour | Implemented | `document timeline::tests::timeline_layers` | O8, O9 |
| Layer folders (nesting, collapse) | Implemented | manual | O8 |
| Guide, mask, masked layers | Implemented | `animation display::tests::masks_and_guides`, `renderer tests::mask_clips_content` | K3 |
| Layer parenting | Implemented | manual | O8 |
| Frame labels (name/comment/anchor) | Implemented | `document io::tests` | — |
| Classic tween (rotation modes, scale, ease) | Verified (span display) / Implemented | `animation tween::tests::classic_tween_midpoint`, `clockwise_rotation_adds_turns` | O17 |
| Shape tween | Implemented | `animation tween::tests::shape_tween_midpoint_between_shapes` | — |
| Motion tween with motion path | Partially compatible | manual | property keyframes inside spans approximated |
| Motion guides, orient to path | Implemented | `animation tween::tests::motion_guide_moves_along_path` | — |
| Easing presets, classic ease, custom curves, per-property eases | Implemented | `document ease::tests` | — |
| Auto keyframe | Implemented | `app ops::tests::timeline_ops_with_auto_keyframe` | O8 |
| Onion skin (range, outlines) | Implemented | manual | O8 |
| Playback, loop, step, go to start/end, scrubbing | Implemented | manual | O10, O15, O16 |
| Nested timelines / graphic-symbol sync / loop modes | Implemented | `animation display::tests::graphic_symbol_sync_and_loop` | K2 |
| Scenes | Implemented | `animation display::tests::camera_view_and_global_frames` | O6 |
| Timeline audio waveforms | Implemented | `media tests` | — |
| Frame scripts / metadata | Implemented | `scripting tests` | — |
| Undo / redo (snapshot history) | Implemented | `document io::tests::undo_redo` | — |

## Symbols & library

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Graphic / movie clip / button symbols | Implemented | `app player::tests`, `renderer tests` | K2 |
| Convert to symbol (registration center/top-left) | Implemented | `app ops::tests::group_ungroup_and_symbol` | — |
| Edit symbol / edit in place (dimmed context, breadcrumb) | Verified | manual | O12, O13 |
| Instance properties (name, behaviour, loop, first frame, visible, cache as bitmap) | Implemented | manual | — |
| Color effects (brightness, tint, alpha, advanced) | Implemented | `document color::tests::tint_and_brightness`, `renderer tests::instance_tint_and_blur` | — |
| Filters: blur, drop shadow, glow, adjust color | Implemented | `renderer filters::tests` | — |
| Filters: bevel, gradient glow/bevel | Not started | — | — |
| Blend modes (normal…erase) | Implemented | manual | Alpha/Erase apply to the canvas below, not a parent "layer" group — Partially compatible |
| Swap symbol, duplicate symbol | Implemented | manual | — |
| Library folders, search, thumbnails, drag & drop | Implemented | manual | O6 |
| Shared / external libraries (import library from .anim/.fla/.xfl) | Implemented | manual | — |
| Asset optimisation | Not started | — | — |

## Character animation & rigging

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Bones and joints (Bone tool, chains) | Implemented | `rigging tests` | — |
| Forward kinematics | Implemented | `rigging tests::forward_kinematics_chain` | — |
| Inverse kinematics (CCD) with angle limits, pinned bones | Implemented | `rigging tests::ik_reaches_target`, `ik_respects_limits` | — |
| Pose keyframes with interpolation | Implemented | `animation tween::pose_at` (manual) | — |
| Element binding to bones | Implemented | `rigging tests::binding_follows_bone` | — |
| Asset warp (pin-based deformation of shapes) | Partially compatible | manual | edits geometry directly; warp tweening not implemented |
| Mesh-based deformation of bitmaps | Not started | — | — |
| Rig mapping / reusable rigs | Partially compatible | — | rigs reusable via symbols and Custom assets |
| Mouth-shape libraries / auto lip sync | Partially compatible | `media tests::lip_sync_detects_silence` | amplitude analysis in library; no UI yet |

## Advanced animation

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Camera layer (pan, zoom, rotate, tint, tweens, camera tool) | Implemented | `animation tween::tests::camera_interpolates`, `display::tests::camera_view_and_global_frames` | O8 |
| Layer depth parallax | Implemented | manual (Parallax sample) | O19 (Layer Depth) |
| Motion presets / animation templates | Partially compatible | manual | Default assets + Custom assets |
| VR view | Not started | — | O19 |

## Typography

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Static / dynamic / input text | Implemented | `importers xfl::tests` | — |
| Font selection (system fonts + bundled fallbacks) | Implemented | `text tests` | — |
| Kerning (OpenType), tracking, leading, alignment, wrapping | Implemented | `text tests::tracking_increases_width`, `layout_produces_glyphs_and_wraps` | — |
| Complex shaping (rustybuzz), bidi / RTL | Implemented | `text tests::newlines_and_rtl` | — |
| Rich text runs (per-run style) | Partially compatible | — | model + rendering support runs; editor edits whole-block style |
| Text to outlines | Implemented | `text tests::text_to_shape_has_paths` | — |
| Font embedding in exports | Implemented as outlines (SVG/Lottie/SWF/HTML5) | — | — |

## Audio & video

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| WAV import; MP3/OGG/FLAC/AAC via FFmpeg | Implemented | `media tests::wav_roundtrip_and_peaks` | — |
| Sync modes (event/start/stop/stream), loops, volume, trim, fades | Implemented | `media tests::stream_sound_is_bounded_by_span` | — |
| Playback in editor and Test Movie | Implemented | manual | uses platform audio player process |
| Native low-latency audio output, scrub audio | Not started | — | — |
| Video import (embedded frames + audio) | Implemented | manual | requires FFmpeg |
| Export with synchronised audio (MP4/WebM/MOV/HTML5/SWF) | Implemented | `exporters tests::video_when_ffmpeg_present` | — |

## Scripting & interactivity

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Script editor with highlighting, snippets, syntax check | Implemented | `scripting tests` | O19 |
| Timeline control API (stop, play, goto, next/prev, scenes) | Implemented | `scripting tests::timeline_commands`, `app player::tests` | — |
| Events (click, enter frame, key, hover) | Implemented | `scripting tests::handlers_and_persistent_variables` | — |
| Clip properties (x, y, alpha, rotation, scale, visible, text) | Implemented | manual | — |
| Test Movie with buttons (Up/Over/Down) | Implemented | manual (scripted click → frame 2) | — |
| ActionScript 3 execution | Partially compatible | `scripting tests::actionscript_compat_and_sandbox` | Rhai with AS-like API; full AS3 semantics not supported |
| Sandboxed plugins (Rhai, no I/O, operation limits) | Implemented | `scripting tests::plugin_modifies_document` | — |
| Debugger (breakpoints) | Not started | — | — |

## File formats

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Native format (versioned, lossless round trip, atomic save) | Implemented | `document io::tests::lossless_roundtrip`, `rejects_future_versions` | — |
| FLA import (CS5+ ZIP) | Partially compatible | `exporters xfl::tests::xfl_roundtrip_through_importer`, `cli tests` | K5; untested with Adobe-made files |
| XFL import / export | Partially compatible | same + `importers xfl::tests::imports_sample_document` | K5–K8 |
| Legacy binary FLA | Blocked | — | proprietary OLE format; explicit error message |
| SVG import / export | Implemented | `importers svg::tests`, `exporters svg::tests` | — |
| PNG / JPEG / GIF / WebP / BMP import | Implemented | `importers tests` | — |
| PSD import | Not started | — | — |
| PNG, PNG sequence, GIF, APNG | Implemented | `exporters tests::gif_apng_png_sequence_and_sheet` | — |
| Animated WebP, MP4, WebM (alpha), MOV ProRes 4444 (alpha) | Implemented | `exporters tests::video_when_ffmpeg_present` | requires FFmpeg |
| SVG sequence, sprite sheet | Implemented | exporters tests | — |
| Lottie JSON export | Partially compatible | `exporters lottie::tests` | baked frames; not validated in a Lottie player here |
| HTML5 Canvas export (interactive subset) | Implemented | `exporters html5::tests` | — |
| SWF export | Partially compatible | `exporters swf::tests` | no filters/blends; not validated in a player here |
| Lottie import, SWF import | Not started | — | — |

## Distribution

| Feature | Status | Tests | Reference evidence |
|---|---|---|---|
| Linux portable tarball (+ per-user install script) | Implemented | built locally, checksum generated | — |
| AppImage, .deb, .rpm, Flatpak, PKGBUILD | Implemented (CI) | release workflow; not built locally (tools unavailable) | — |
| Windows MSI (WiX), setup EXE (NSIS), portable zip, x64 + arm64 | Implemented (CI) | release workflow; not built locally | — |
| macOS .app, DMG, PKG, universal binary, tarball, Homebrew cask, terminal installer, signing/notarization hooks | Implemented (CI) | release workflow; not built locally | — |
| GitHub Actions CI (fmt, clippy -D warnings, tests on 3 OSes) and release pipeline with SHA-256 | Implemented | not executed in this session | — |
