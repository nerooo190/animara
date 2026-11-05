# Timeline analysis

## Observed (R1)

* Layers are listed top-to-bottom with a colour swatch, visibility (eye) and lock columns (O8, O9).
* Keyframes are drawn as filled dots; extended frames as grey spans; a classic tween as a **purple** span with an
  arrow between keyframes (O17).
* The timeline header shows the frame rate (29.97 FPS) and current frame ("1 F"), insert keyframe / blank keyframe /
  frame buttons, an auto-keyframe toggle, onion skin, loop and playback controls (O8).
* The ruler shows frame numbers every 5 frames and second markers (O10).
* Entering a symbol replaces the timeline with the symbol's own layers (O12).

## Model chosen for Animara

* A layer is a **contiguous list of keyframe spans** starting at frame 0 (`Keyframe { start, duration }`), which
  makes F5/F6/F7/Shift+F5/Shift+F6, copy/paste/reverse frames and keyframe dragging simple, well-defined operations
  (`crates/document/src/timeline.rs`, 6 unit tests for the editing semantics).
* Tweens belong to the keyframe that starts the span and interpolate toward the next keyframe:
  * **Classic** — decomposed transform interpolation about the transformation point, rotation modes
    (auto / none / CW / CCW + extra turns), colour effects and filters, motion guides with orient-to-path.
  * **Shape** — contour correspondence (subdivision to equal segment counts, orientation and start-point alignment),
    fill/stroke/gradient interpolation.
  * **Motion** — classic interpolation plus an editable motion path; when no end keyframe exists the element follows
    the path over the span. Property keyframes *inside* a span (Animate's motion-tween property keyframes) are
    approximated by ordinary keyframes — **Partially compatible**.
* Easing: classic strength, 27 named presets, custom Bézier curves, and per-property eases (position, rotation,
  scale, color, filters) edited in the Motion Editor.
* Auto keyframe: editing an object on an interpolated frame inserts a keyframe with the interpolated state.
* Graphic symbols follow the parent playhead (loop / play once / single frame / reverse variants);
  movie clips advance independently in Test Movie and exports; buttons show Up/Over/Down via the player.
* Onion skinning (before/after counts, tinted, optional outlines).
* Audio: sounds attach to keyframes (event/start/stop/stream sync, loops, volume, trim, fades); waveforms are drawn
  in the layer row.

## Differences / unknowns

| Topic | Status |
|---|---|
| Exact colours of Animate's motion/shape tween spans | Inferred (classic purple observed; motion amber, shape green chosen) |
| Frame-based selection vs span-based selection preferences | Unknown — Animara uses frame-based selection |
| Layer depth panel UI | Not implemented as a panel; depth is a layer property |
| Scale Spans option | Not implemented |
