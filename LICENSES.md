# Licenses

Animara is dual-licensed under **MIT OR Apache-2.0**. All Animara source code, icons (drawn procedurally in
`crates/app/src/icons.rs`), the application icon (`assets/animara.svg`), sample documents (generated in code) and
documentation are original works. No Adobe code, artwork, icons, fonts, templates or sample files are included.

## Third-party Rust crates (main direct dependencies)

| Crate | License | Use |
|---|---|---|
| eframe, egui, epaint | MIT OR Apache-2.0 | GUI |
| epaint_default_fonts | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | Bundled UI fonts (Ubuntu Light, Hack) and text fallback |
| egui_stack | MIT | Docking (continuation of egui_dock) |
| tiny-skia | BSD-3-Clause | Rasterization |
| usvg | Apache-2.0 OR MIT | SVG import |
| rustybuzz, ttf-parser | MIT / Apache-2.0 OR MIT | Text shaping and outlines |
| fontdb | MIT | Font discovery |
| unicode-bidi | MIT OR Apache-2.0 | Bidirectional text |
| i_overlay | MIT | Polygon boolean operations |
| rhai | MIT OR Apache-2.0 | Scripting |
| quick-xml | MIT | XFL parsing |
| zip | MIT | Native format and FLA containers |
| image, png, gif | MIT OR Apache-2.0 | Raster codecs |
| hound | Apache-2.0 | WAV |
| miniz_oxide | MIT OR Zlib OR Apache-2.0 | zlib for FLA bitmap data |
| serde, serde_json | MIT OR Apache-2.0 | Serialization |
| rfd | MIT | Native file dialogs |
| base64, dirs, thiserror, log | MIT OR Apache-2.0 | Utilities |

Run `cargo tree` for the full transitive list. For a release SBOM, use `cargo about` or `cargo deny check licenses`.

## External programs (not distributed)

* **FFmpeg** (LGPL-2.1+/GPL depending on build) is invoked as a separate process when present on the user's
  system. It is not linked or bundled.
* Platform audio players (`pw-play`, `paplay`, `aplay`, `ffplay`, `afplay`, PowerShell) are invoked as processes.
* **Zig** (MIT) is used only as a development-time linker on hosts without a C toolchain (`tools/dev-env.sh`). It is
  not part of any artifact.

## Trademarks

Adobe and Adobe Animate are trademarks of Adobe Inc. They are mentioned only to describe file-format
interoperability.
