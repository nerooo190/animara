# Evidence log

Every compatibility claim in this repository is classified as one of:

| Class | Meaning |
|---|---|
| **Observed** | Directly observed in the reference material listed below. |
| **Documented** | Supported by public documentation (cited where used). |
| **Inferred** | Derived from general knowledge of the product/format or from indirect evidence; not confirmed in this project against the real application or real files. |
| **Unknown** | Not determined. |

## Reference material available

| ID | Material | Details |
|---|---|---|
| R1 | `adobe animate reference.mp4` (project root, not committed) | 207.7 s, 1280×720 H.264 @ 30 fps, AAC 44.1 kHz stereo, 6 750 251 bytes, SHA-256 `e71fbd26c7b7f7c770071a70d08c61ff7ed121ee6d11757558fc256dfa013d81`. Screen recording of Adobe Animate 2023 on Windows 10/11 (taskbar date 19 Feb 2024). |

**Not available:** no Adobe Animate installation, binaries, `.fla`/`.xfl` sample documents, SWF files or official
specification documents were present on the analysis machine. Consequently:

* The *Reverse Engineer Anything* (REA) skill was loaded and its tool inventory reviewed. REA targets shipped
  artifacts (native/managed binaries, packages, web bundles, captures). With no Animate artifact available and only a
  video recording as reference, there was no binary target to open; REA analysis is **not applicable** for this
  session. No licensing, authentication or encryption controls were bypassed, and no Adobe code, artwork or
  resources were copied.
* All file-format work (XFL/FLA/SWF) is **clean-room**, written from general format knowledge and validated by
  round-tripping through this project's own exporter and importer. It has **not** been validated against files
  produced by Adobe Animate. See `format-compatibility.md`.

## Method

Frames were extracted with FFmpeg every 6 s (`fps=1/6`) into 2×2 contact sheets with burned-in timestamps, plus
full-resolution stills at 66 s, 96 s, 165 s, 186 s and 198 s and zoomed crops of the tools panel and timeline header.
Observations below cite the video timestamp (mm:ss).

## Observations from R1

| # | Time | Observation | Class | Used in |
|---|---|---|---|---|
| O1 | 00:18 | Launched from the Windows start menu as "Adobe Animate 2023". | Observed | — |
| O2 | 00:30–00:36 | Startup shows an empty dark workspace with menu bar *File, Edit, View, Insert, Modify, Text, Commands, Control, Debug, Window, Help*, then a splash/blank window. | Observed | Menu bar order (`menus.rs`) |
| O3 | 00:42–00:48 | **New Document** dialog: category tabs *Character Animation, Social, Game, Education, Ads, Web, Advanced*; "PRESETS (4)" grid *HD 1280×720, Full HD 1920×1080, 4K 3840×2160, Standard 640×480*; DETAILS panel with *Width, Height, Units (Pixels), Frame Rate 30.00, Platform Type (ActionScript 3.0)*; blue **Create** button; "SAMPLE FILES (14)" strip below. | Observed | `dialogs.rs` New Document, `TEMPLATES` |
| O4 | 00:54 | **New from Template** dialog: Category list (*Advertising, AIR for Android, AIR for iOS, Animation, Banners, HTML5 Canvas, Media Playback, Presentations, Sample Files*), template list of ad sizes (88×31 Microbar … 300×250 Medium Rectangle), preview and description boxes. | Observed | Ads category presets |
| O5 | 01:06 | **File** menu: New (Ctrl+N), New from Template… (Ctrl+Shift+N), Open (Ctrl+O), Browse in Bridge (Ctrl+Alt+O), Open Recent ▸, Close (Ctrl+W), Close All (Ctrl+Alt+W), Save (Ctrl+S), Save As… (Ctrl+Shift+S), Save as Template…, Save All, Revert, Import ▸, Export ▸, Convert To ▸, Publish Settings… (Ctrl+Shift+F12), Publish (Alt+Shift+F12), AIR Settings…, ActionScript Settings…, Exit (Ctrl+Q). | Observed | `commands.rs` shortcuts |
| O6 | 01:24 | Main workspace ("Essentials"-like): tools column left; stage centre with edit bar (*Scene 1* breadcrumb, scene/symbol buttons, zoom combo *64%*); **Timeline / Output** tabs below the stage; right panel tabs *Assets, Properties, Color, Align, Library*. | Observed | `app.rs` default layout |
| O7 | 01:24, crop | Tools column order: Selection, Free Transform, Lasso, Fluid/Paint Brush, Classic Brush, Eraser, Rectangle, Line, Pen, Text, Paint Bucket, Eyedropper, Asset Warp (pin), Hand, Zoom, "…" (edit toolbar), stroke/fill swatches. | Observed | `Tool::TOOLBAR` (grouping), icons are original artwork |
| O8 | 01:24, crop | Timeline header: layer-panel toggle, camera, layer parenting, graph (motion editor) icons; FPS readout *29.97 FPS*; current frame *1 F*; insert keyframe / blank keyframe / frame buttons; auto-keyframe toggle; onion-skin button; loop button; playback controls (rewind, step back, play, step forward); frame-size slider. Layer list header: new layer, new folder, delete, highlight dot, outline, show/hide (eye), lock. | Observed | `timeline_ui.rs` header |
| O9 | 01:24 | Layer rows show a colored outline swatch per layer (green, etc.), eye and lock columns; keyframes as dots; spans grey. | Observed | Layer row rendering |
| O10 | 01:30–01:42 | Scrubbing the playhead: frame counter updates (5 F, 12 F, 15 F); time ruler shows seconds markers ("1s", "2s"). | Observed | Ruler second markers |
| O11 | 01:48 | Selecting an object on the stage shows a light-blue bounding box with a transformation point (circle) and registration cross. | Observed | Selection overlays |
| O12 | 01:54–02:00 | Double-clicking an instance enters symbol editing in place: breadcrumb becomes *Scene 1 › Dog_rig_sitting*; other content is dimmed; the timeline shows the symbol's layers (dog_ear1, dog_head, dog_ear2, dog_leg1) with different colors. | Observed | Edit-in-place, breadcrumb, dimmed parent |
| O13 | 02:06 | Returning to Scene 1 restores the scene timeline. | Observed | Breadcrumb navigation |
| O14 | 02:12–02:48 | Brush tool selected; freehand blue strokes drawn on the stage; frame counter advances while drawing new strokes on subsequent frames (30 F, 32 F, 33 F, 41 F) — new keyframes added as dots on the *Shadow* layer. | Observed | Brush tool, keyframe insertion |
| O15 | 02:42 | Playback with Play button: frame counter 1 F → continues; playhead moves. | Observed | Playback |
| O16 | 02:54 | Timeline scrolled to frames 160–230; playhead at 218 F; Pause icon shown while playing. | Observed | Play/Stop toggle icon |
| O17 | 03:00–03:24 | A **classic tween** span is displayed as a purple span between keyframes 265–296 with an arrow; frame 281 F shows the tweened pose. | Observed | Tween colors (classic = purple) |
| O18 | 03:06–03:24 | Properties panel with **Tool / Object / Frame / Doc** sub-tabs; Doc tab: document name, *Publish Settings* (Profile: Default, More settings), *Document Settings* (W 1920, H 1080, Match contents, Scale Content, Apply to pasteboard, Stage color swatch, FPS 29.97, Scale Spans), *SWF History*, *Accessibility*. | Observed | `panels::properties` tabs, Doc tab |
| O19 | 03:18 | **Window** menu: Duplicate Window (Ctrl+Alt+K), Find Extensions on Exchange…, Extensions ▸, Edit Bar, Timeline (Ctrl+Alt+T), Tools (Ctrl+F2), Properties (Ctrl+F3), Assets, Rig Mapping, CC Libraries, Library (Ctrl+L), Brush Library, Motion Presets, VR View, Frame Picker, Layer Depth, Actions (F9), Code Snippets, Compiler Errors (Alt+F2), Debug Panels ▸, Output (F2), Align (Ctrl+K), Color (Ctrl+Shift+F9), Info (Ctrl+I), Swatches (Ctrl+F9), Transform (Ctrl+T), Components (Ctrl+F7), Component Parameters, History (Ctrl+F10), Scene (Shift+F2), Workspaces ▸, Hide Panels (F4), document list. | Observed | Window menu items and shortcuts |
| O20 | 01:24 | Assets panel: *Default / Custom* tabs, search, *Animated* section with Characters/Objects filters, thumbnail grid, *Static* and *Sound clips* sections. | Observed | `panels::assets_panel` (built-in procedurally generated assets; no Adobe assets) |
| O21 | 01:24 | Pasteboard area around the stage is a light grey; stage white; zoom 64%. | Observed | Stage/pasteboard colors (themed) |

## Product/format knowledge used (not verified here)

| # | Statement | Class | Used in |
|---|---|---|---|
| K1 | F5 insert frame, Shift+F5 remove frames, F6 keyframe, Shift+F6 clear keyframe, F7 blank keyframe, F8 convert to symbol, Ctrl+Enter test movie, Enter play/stop. | Inferred | `commands.rs` |
| K2 | Graphic symbols synchronise to the parent timeline with Loop / Play Once / Single Frame options; movie clips run their own timeline; buttons use Up/Over/Down/Hit frames. | Inferred | animation engine, player |
| K3 | Mask layers clip the layers indented beneath them and are applied in authoring only when locked. | Inferred | `display.rs` |
| K4 | Classic tween ease is a −100…100 strength (negative = ease in). | Inferred | `Ease::Classic` |
| K5 | XFL = folder with `DOMDocument.xml`, `LIBRARY/*.xml`, `bin/`, `META-INF/`; CS5+ FLA = ZIP of that folder; legacy FLA = OLE compound document. | Inferred | `importers::xfl`, `exporters::xfl` |
| K6 | XFL shape edges: `!` moveTo, `|`/`/` lineTo, `[` quadTo, coordinates in twips (1/20 px), optional `#hex.frac` form; `fillStyle0/1` = left/right fill. | Inferred | `parse_edges`, edge reconstruction |
| K7 | XFL gradient matrices map a ±16384-twip (±819.2 px) square. | Inferred | `GRADIENT_HALF` |
| K8 | Lossless bitmap `bin/*.dat` layout (header 0x0503, ARGB premultiplied zlib chunks). | Inferred, low confidence | `bitmap_from_dat` (fallback only) |
| K9 | SWF file structure (FWS header, RECORDHEADER, DefineShape3, PlaceObject2, DefineBitsLossless2, SoundStreamHead2/Block). | Inferred (public SWF spec, not re-read in this session) | `exporters::swf` |
| K10 | Lottie/Bodymovin JSON schema 5.x (layers, shapes `gr/sh/fl/st/gf/tr`, masks, effects). | Inferred | `exporters::lottie` |
