# Feature inventory

The inventory enumerates the capability areas requested for Animate-class parity. Status, tests and evidence for
each row live in [`FEATURE_PARITY.md`](../../FEATURE_PARITY.md); evidence IDs are defined in
[`evidence-log.md`](evidence-log.md).

| Area | Sub-features | Evidence |
|---|---|---|
| Workspace | menu bar, tools, stage + edit bar, timeline, docked panels, Properties Tool/Object/Frame/Doc, workspaces, themes, shortcuts, command search | O2, O6–O9, O18–O20 |
| Documents | new (presets by category), templates, open/save/close/revert, recent, multi-document tabs, autosave/recovery | O3–O5 |
| Drawing | selection, subselection, free transform, gradient transform, lasso, pen, pencil, paint/fluid brush, classic brush, eraser, line, rectangle, oval, polystar, primitives, bucket, ink bottle, eyedropper, text, asset warp, bone, camera, hand, zoom | O7, O11, O14 |
| Vector model | Bézier paths, fills (solid/linear/radial/bitmap), strokes (styles, caps, joins), boolean combine, smoothing/straightening, lines→fills, snapping, guides, rulers, grid, align/distribute/transform | K-series |
| Timeline | layers, folders, guide/mask/masked, camera, parenting, frames/keyframes/blank, labels, scripts, copy/paste/reverse/convert frames, onion skin, playback, loop, scrubbing | O8–O10, O14–O17 |
| Tweens | classic, shape, motion (path), easing presets/custom/per-property, rotation, motion guides | O17, K4 |
| Symbols | graphic / movie clip / button, nested, edit in place, library (folders, search, thumbnails, drag & drop), swap, duplicate, color effects, filters, blend modes | O12, O13, O20, K2 |
| Rigging | bones, FK, IK (CCD + limits), pose keyframes, binding, asset warp (pins) | — |
| Text | static/dynamic/input, fonts, size, color, bold/italic, tracking, kerning, leading, alignment, wrapping, RTL/bidi, shaping, break apart / outlines | — |
| Audio/video | import, waveforms, sync modes, loops, volume, trim, fades, playback, video embedding, export with audio, lip-sync assistance (library) | — |
| Scripting | frame scripts, events (click, enter frame, keys, hover), clip control, trace, Test Movie, syntax check, snippets, plugins | O19 (Actions/Code Snippets/Compiler Errors exist) |
| Formats | native, FLA/XFL in/out, SVG in/out, images, GIF/APNG/WebP, MP4/WebM/MOV, sprite sheet, Lottie, HTML5, SWF | K5–K10 |
| Distribution | Linux (tarball, AppImage, deb, rpm, Flatpak, PKGBUILD), Windows (MSI, EXE, portable zip), macOS (app, DMG, PKG, tarball, Homebrew, terminal installer), CI | — |
