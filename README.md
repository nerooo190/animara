# Animara

Animara is a timeline-based 2D vector animation studio written in Rust. It follows the workflow of professional
animation tools: vector drawing, layered timelines, symbols, tweens, bones, cameras, scripting, and export to video
and web formats. It also opens Adobe Animate CS5+ FLA and XFL documents.

> Animara is independent software. It is not affiliated with or endorsed by Adobe. All code, icons and artwork are
> original. "Adobe" and "Animate" are trademarks of Adobe Inc., used here only to describe file compatibility.

## Highlights

* **Drawing:** selection, subselection, free and gradient transform, lasso, pen, pencil, pressure/velocity brush,
  classic brush, eraser, shapes and editable primitives, paint bucket, ink bottle, eyedropper, text, asset warp,
  bones, camera, plus booleans, smoothing, snapping, rulers, guides and grid.
* **Timeline:** layers, folders, guide/mask layers, layer parenting, keyframes, labels, classic/shape/motion tweens
  with easing presets and custom curves, motion guides, onion skin, scenes, audio with waveforms.
* **Symbols:** graphic, movie clip and button symbols with edit-in-place, color effects, filters and blend modes,
  plus a searchable library with drag and drop.
* **Rigging:** bones with forward and inverse kinematics (with angle limits) and interpolated pose keyframes.
* **Scripting:** sandboxed [Rhai](https://rhai.rs) frame scripts with an ActionScript-like API, event handlers,
  Test Movie, and plugin commands.
* **Formats:** `.anim` (native), FLA/XFL import and export, SVG import and export, images, GIF, APNG, WebP,
  MP4/WebM/MOV (via FFmpeg), sprite sheets, Lottie, HTML5 Canvas and SWF.
* **Two binaries:** `animara` (the GUI) and `animara-cli` (headless render, export, convert, validate and bench).

## Quick start

```bash
cargo run --release -p animara-app            # GUI
cargo run --release -p animara-cli -- --help  # CLI
```

The GUI opens with a welcome screen offering presets and five sample documents. See [BUILDING.md](BUILDING.md) for
prerequisites, and Help ▸ Animara Help (F1) for tool shortcuts and the scripting API.

## Documentation

| Document | Contents |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Crates, data model, rendering, design decisions |
| [FEATURE_PARITY.md](FEATURE_PARITY.md) | Feature-by-feature status with tests and evidence |
| [COMPATIBILITY.md](COMPATIBILITY.md) | File-format compatibility and known limitations |
| [ROADMAP.md](ROADMAP.md) | Next milestones |
| [BUILDING.md](BUILDING.md), [PACKAGING.md](PACKAGING.md), [TESTING.md](TESTING.md) | Developer guides |
| [docs/reverse-engineering/](docs/reverse-engineering/) | Evidence log and reference analysis |
| [LICENSES.md](LICENSES.md), [CHANGELOG.md](CHANGELOG.md) | Licensing and history |

## License

MIT OR Apache-2.0. Third-party components are listed in [LICENSES.md](LICENSES.md).
