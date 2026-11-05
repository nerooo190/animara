# Architecture

## Workspace

```text
crates/
├── geometry     Vec2, Affine, Rect, Bézier Path (SVG data, flatten, hit test, sampling, anchor editing),
│                shape morphing, boolean ops (i_overlay), smoothing, variable-width brush outlines
├── document     Document → Scene → Timeline → Layer → Keyframe → Element; Library (symbols, bitmaps, sounds);
│                colors/fills/strokes/color effects; easing; timeline editing ops; native .anim I/O; undo history
├── rigging      Armature FK, constrained CCD IK, pose interpolation, element binding
├── animation    Tween interpolation (classic/shape/motion), motion guides, poses, cameras, layer parenting,
│                graphic-symbol sync; flattening into a DisplayItem tree (masks, groups with filters/blend)
├── text         Font discovery (fontdb), shaping (rustybuzz), bidi (unicode-bidi), wrapping, glyph outlines
├── renderer     tiny-skia CPU renderer for DisplayLists: gradients, bitmaps, strokes, filters, blend modes,
│                masks; bounds and hit testing
├── media        WAV decode/encode, waveform peaks, timeline mixdown, platform audio playback, FFmpeg bridges
│                (audio/video import), lip-sync analysis
├── importers    Images, SVG (usvg), XFL/FLA
├── exporters    PNG/sequence/GIF/APNG/sprite sheet, video via FFmpeg, SVG, Lottie, HTML5 Canvas, SWF, XFL/FLA
├── scripting    Rhai host with timeline API, events, sandbox limits, plugin runner
├── app          egui/eframe desktop application (`animara`)
└── cli          headless tool (`animara-cli`)
```

The dependency direction is strictly downward: `app` and `cli` sit on top. `renderer` depends on `animation` and
`text`. `document` depends only on `geometry`. The engine crates are GUI-free and are reused by the CLI and the
exporters.

## Data model

* **Keyframe spans.** Each layer stores contiguous `Keyframe { start, duration, elements, tween, label, script,
  sound, camera, pose }` spans starting at frame 0. Timeline operations (F5/F6/F7…) are pure functions on this list,
  and `Layer::validate` checks the invariants.
* **Elements** are `Shape` (paths with fill/stroke), `Instance` (symbol reference plus color effect, filters, blend
  and loop mode), `Text` (styled runs), `Bitmap`, or `Group`. Each element has an affine transform and a
  transformation point (`pivot`).
* **Identity:** every object has a process-unique `Id`. Tweens match elements across keyframes by id (falling back
  to index), so keyframe copies keep ids while paste and duplicate assign new ones.
* **Undo:** snapshot history of the whole `Document`. Media is shared through `Arc`, so snapshots stay cheap.
  Snapshots were chosen for reliability over a command-diff log.

## Frame evaluation and rendering

1. `animation::evaluate_document_frame` resolves tweens, poses, cameras and nested symbols into a `DisplayList`:
   a tree of shapes, text, bitmaps, groups (for filters, blend modes and bitmap caching) and masked groups.
2. `renderer::Renderer` draws the list with tiny-skia. Groups with filters or blend modes are rendered off-screen
   and composited. Masks use the mask's alpha coverage.
3. The stage renders the visible viewport at the physical pixel resolution into an egui texture, cached by a
   content key (revision, frame, view, onion settings). Selection, handles, guides, rulers, bones and motion paths
   are drawn as egui overlays.
4. Exporters use the same evaluation and renderer, so stage and export output match. Raster exports render frames
   in parallel batches, one worker per core.

### Why a CPU renderer?

tiny-skia gives deterministic, high-quality anti-aliasing that is identical on every platform and in headless CI.
That makes pixel-comparison tests possible. A GPU path (Vello/wgpu) is on the roadmap for very heavy scenes.
Measured costs are in [TESTING.md](TESTING.md#benchmarks).

## GUI

* eframe/egui 0.37 with the `glow` backend; Wayland and X11 on Linux.
* Docking uses **egui_stack 0.22** (MIT), a maintained continuation of egui_dock 0.21.1 ported to egui 0.37, with
  the same API. egui_dock 0.21 still depends on egui 0.36.
* Commands are a single `Command` enum shared by menus, shortcuts and the command palette. Shortcuts are
  user-configurable and persisted.
* Settings, workspaces, autosave and recovery files live in the platform config directory (`dirs`).

## Scripting & plugins

Rhai is embedded with no I/O functions, an operation limit, and call-depth and size limits. Frame scripts enqueue
`ScriptCmd`s that the Test Movie player applies. Plugins receive the document as a Rhai map. Their output is
deserialized and validated before it replaces the document, and media bytes are never exposed to plugins.

## External tools

FFmpeg is used as an **external process** for video and non-WAV audio, so it is never linked. Audio playback uses
the platform's command-line player (`pw-play`/`paplay`/`aplay`/`ffplay`, `afplay`, PowerShell `SoundPlayer`). This
keeps builds free of native audio headers; a native cpal backend is on the roadmap.

## Unsafe code

The workspace contains **no `unsafe` blocks**.
