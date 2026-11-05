# Changelog

## 0.1.0 — 2026-11-05

First release.

* Rust workspace with 12 crates: geometry, document, rigging, animation, text, renderer, media, importers,
  exporters, scripting, app, cli.
* Desktop application with docking workspace, 23 tools, timeline, library, Properties (Tool/Object/Frame/Doc), color,
  swatches, align, transform, history, actions, motion editor, scenes, info, components, assets, output panels;
  command palette; custom shortcuts and workspaces; dark/light themes; autosave and crash recovery.
* Animation engine: classic/shape/motion tweens, easing presets and custom curves, motion guides, graphic symbol
  sync, masks, cameras with parallax, bones with FK/IK, onion skin, scenes, audio.
* Test Movie player with Rhai scripting, buttons, movie clips and events; sandboxed plugins.
* Import: `.anim`, FLA (CS5+), XFL, SVG, PNG/JPEG/GIF/WebP/BMP, animated GIF, WAV (+ MP3/OGG/FLAC/AAC and video via
  FFmpeg).
* Export: PNG, PNG sequence, GIF, APNG, WebP, MP4, WebM, MOV, SVG, SVG sequence, sprite sheet, Lottie, HTML5 Canvas,
  SWF, XFL, FLA.
* `animara-cli`: info, validate, check-scripts, render, export, convert, bench.
* Packaging definitions and GitHub Actions release pipeline for Linux, Windows and macOS.
