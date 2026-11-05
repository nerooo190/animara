# Compatibility

Detailed per-construct tables: [docs/reverse-engineering/format-compatibility.md](docs/reverse-engineering/format-compatibility.md).

## Summary

* **Not 100 % compatible.** Animara aims for workflow and file compatibility with Adobe Animate. That has been
  checked only against a screen recording of Animate 2023 and this project's own fixtures. No Adobe-generated FLA,
  XFL or SWF files were available for validation.
* **FLA/XFL import** covers document settings, scenes, layer types and relationships, frames, labels, scripts,
  classic and shape tweens, shapes (edge reconstruction), primitives, instances (color effects, filters, blend
  modes), text, groups, library folders, bitmaps and sounds with source files. Anything unsupported produces a
  warning in the Output panel or on stderr; nothing is dropped silently.
* **Legacy binary FLA** (Flash CS4 and earlier) is rejected with an explanation. Re-save such files as XFL or CS5+
  FLA in the original application.
* **ActionScript:** frame scripts are imported as text and run on the Rhai runtime. The timeline API is compatible
  (`stop`, `play`, `gotoAndPlay/Stop`, `nextFrame`, …, `var x:Type =` declarations). General AS3 (classes,
  display list API, events beyond click/enter-frame/key/hover) is not.
* **Rendering differences to expect:** Animate's merge-drawing shapes are imported as separate filled and stroked
  paths. Stroke joins and caps follow tiny-skia. Blur quality uses box-blur passes. `Alpha`/`Erase` blend modes
  composite against everything already drawn rather than a parent "Layer" group.
* **Video/audio** formats other than WAV need FFmpeg on `PATH`.
