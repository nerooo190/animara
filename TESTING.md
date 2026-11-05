# Testing

```bash
cargo test --workspace                              # 84 tests (80 unit + 4 CLI end-to-end)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

## Test inventory

| Area | Location | What is checked |
|---|---|---|
| Geometry | `crates/geometry` (12) | SVG path round trip, arcs, containment/distance, sampling, shape morph endpoints, anchor editing, boolean areas, RDP, smooth fit, brush outline |
| Document | `crates/document` (15) | Timeline editing semantics (F5/F6/F7/clear/remove/copy/paste/reverse/move keyframe), color/HSV/tint, easing endpoints, lossless `.anim` round trip, future-version rejection, undo/redo |
| Rigging | `crates/rigging` (4) | FK chain, IK reaches target, IK angle limits, bone binding |
| Animation | `crates/animation` (8) | Classic tween midpoint/rotation turns, shape tween, motion guide, camera interpolation, graphic symbol looping, masks & guides in display lists, camera view & global frames |
| Text | `crates/text` (4) | Glyph layout and wrapping, tracking, newlines + RTL, text-to-outline |
| Renderer | `crates/renderer` (6) | Pixel checks: solid fill/background/scale/transparency, linear gradient, instance tint + blur, mask clipping, blur energy, drop-shadow offset |
| Media | `crates/media` (3) | WAV round trip + peaks, stream-sync bounds in the mixdown, lip-sync silence detection |
| Scripting | `crates/scripting` (4) | Timeline commands, handlers + persistent variables, ActionScript-style compatibility and sandbox limits, plugin execution |
| Importers | `crates/importers` (6) | XML parser, BMP→PNG, SVG shapes/gradients, twips/edges parsing, full XFL sample import |
| Exporters | `crates/exporters` (11) | GIF/APNG frame counts, PNG sequence, sprite-sheet JSON, MP4/WebM via FFmpeg (skipped if absent), SVG re-import, Lottie structure, HTML5 actions + data, SWF tag stream integrity, stored zlib, **XFL/FLA round trip with pixel comparison** |
| App | `crates/app` (7) | Align/distribute + undo, group/ungroup/convert-to-symbol, boolean combine, auto-keyframe on tweened frames, shortcut uniqueness/parsing, Test Movie `gotoAndStop(label)` |
| CLI end-to-end | `crates/cli/tests/cli.rs` (4) | Runs the real binary on `tests/fixtures/xfl/bouncing`: info/validate/check-scripts, rendered pixel colors, every exporter writes output, XFL→.anim→FLA conversion |

## GUI verification

The GUI was exercised on a Steam Deck (KDE Plasma, Wayland and XWayland) by launching `animara --sample N` and
driving it with `xdotool`. Screenshots confirmed:

* the workspace layout, welcome screen and sample documents;
* the rectangle and oval tools creating filled and stroked shapes, with the document marked modified;
* the Interactive Buttons sample in Test Movie: `stop()` on frame 1, then clicking the button ran the `on_click`
  handler, which called `gotoAndStop(2)` and showed the button's Over state.

There is no automated GUI test harness yet (see ROADMAP).

## Benchmarks

`animara-cli bench --layers L --shapes S --frames F` builds a synthetic document: L layers × S tweened elements,
a third of them two-level nested symbol instances with radial gradients and strokes. It measures evaluation, CPU
rendering at 1920×1080, and saving and loading. Reproduce with a release build.

Measured on a Steam Deck (AMD Custom APU 0405, 4 cores / 8 threads), release build, 5 Nov 2026:

| Scenario | Evaluate | Render 1080p | Save | Load |
|---|---|---|---|---|
| 5 × 20 elements (100 per frame) | 0.062 ms | 8.5 ms (118 fps) | 2.0 ms | 2.3 ms |
| 20 × 50 elements (1 000 per frame) | 1.14 ms | 87.2 ms (11.5 fps) | 16.7 ms | 23.4 ms |

PNG-sequence export of the 1 000-element document (60 frames): **8.43 s** sequential → **3.75 s** with parallel
frame rendering (2.25×; PNG encoding remains serial).
