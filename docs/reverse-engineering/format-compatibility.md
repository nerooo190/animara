# Format compatibility

No Adobe-produced FLA/XFL/SWF files were available, so every row below is validated only against this project's
own writers (round trip) and hand-written fixtures (`tests/fixtures/xfl/bouncing`). Classes follow `evidence-log.md`.

## XFL / FLA import (`crates/importers/src/xfl.rs`)

| Construct | Support | Class |
|---|---|---|
| ZIP-based FLA (CS5+) | Read via the `zip` crate | Inferred |
| Legacy binary FLA (OLE, CS4 and earlier) | Rejected with an explicit message | Inferred |
| `DOMDocument` width/height/frameRate/backgroundColor | Yes | Inferred |
| Scenes (`<timelines>/<DOMTimeline>`) | Yes | Inferred |
| Layers: normal, guide, mask, folder, camera; visible/locked/outline/open, color, `layerDepth` | Yes | Inferred |
| `parentLayerIndex` → folder / mask / motion guide / layer parent | Yes | Inferred |
| Frames: index, duration, labels (name/comment/anchor), ActionScript CDATA, sounds | Yes (scripts kept as text) | Inferred |
| Classic tweens (`tweenType="motion"`), acceleration, rotation, orient to path, scale | Yes | Inferred |
| Custom ease curves (`<tweens>`) | Approximated by acceleration, warning emitted | Inferred |
| Motion-object tweens (`motionObjectXML`) | Approximated, warning emitted | Inferred |
| Shape tweens, shape hints | Tweens yes; hints no | Inferred |
| `DOMShape` fills (solid, linear/radial gradient, bitmap) and strokes (solid, dashed, dotted, hairline; others → solid + warning) | Yes | Inferred |
| Edge format (`!`, `|`, `/`, `[`, `]`, `S#`, hex coordinates) and fill reconstruction from fillStyle0/1 | Yes | Inferred |
| Rectangle / oval primitives | Yes (corner radius, angles, inner radius) | Inferred |
| Symbol instances: matrix, transformation point, color effects (brightness/tint/alpha/advanced), blend modes, filters (blur, drop shadow, glow, adjust color), loop/first frame, instance name, visibility, cacheAsBitmap | Yes; bevel/gradient filters → warning | Inferred |
| Groups, bitmap instances | Yes | Inferred |
| Static / dynamic / input text (runs, face, size, color, alignment, letter spacing, line spacing, URL) | Yes; TLF → classic text + warning | Inferred |
| Library folders, symbol items (graphic/movie clip/button, linkage) | Yes | Inferred |
| Bitmaps | From `LIBRARY/<href>` source files; `bin/*.dat` lossless decoding is a low-confidence fallback | Inferred |
| Sounds | From `LIBRARY/<href>` source files only; otherwise warning | Inferred |
| Video items, components, persistent data, instance-level `on(...)` scripts | Warning, skipped | Inferred |

## XFL / FLA export (`crates/exporters/src/xfl.rs`)

Writes everything the importer reads (above). Curves are written as quadratic edges (cubics approximated).
Round-trip test: export → import → structure equality + mean pixel difference < 2/255 on a rendered frame.

## Other formats

| Format | Import | Export | Notes |
|---|---|---|---|
| Native `.anim` | Yes | Yes | ZIP: `mimetype`, `document.json` (versioned, `format_version` 1), `media/<id>.<ext>`; atomic save; lossless round trip test |
| SVG | Yes (usvg: paths, gradients, groups, opacity, images, text → outlines; clip/mask/filters → warning) | Frame or sequence (gradients, patterns, filters, blend modes, masks) | Re-import of exported SVG is tested |
| PNG / JPEG / GIF / WebP / BMP | Yes (BMP re-encoded to PNG; animated GIF → frame-by-frame) | PNG frame, PNG sequence, animated GIF, APNG, animated WebP (FFmpeg) | |
| MP4 / WebM / MOV | Video import → bitmap frames + audio (FFmpeg) | H.264 MP4, VP9 WebM (alpha), ProRes 4444 MOV (alpha) with mixed audio (FFmpeg) | Requires FFmpeg on PATH |
| WAV | Yes (native) | Mixed timeline audio | |
| MP3 / OGG / FLAC / AAC | Yes via FFmpeg | — | |
| Lottie JSON | — | Yes (baked runs, gradients, masks, blur effect, image layers) | Validated structurally; not validated in lottie-web in this session |
| HTML5 Canvas | — | Self-contained HTML + JS player (actions, click handlers, audio) | Script subset: stop/play/goto/next/prev + on_click |
| SWF | — | FWS v10: DefineShape3, gradients, bitmaps, clip masks, streamed PCM | Not validated in a Flash player/Ruffle in this session |
| Sprite sheet | — | PNG + JSON (hash format) | |
| PSD | — | — | Not started |
| Lottie import, SWF import | — | — | Not started |
