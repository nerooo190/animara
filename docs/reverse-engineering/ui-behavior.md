# UI behaviour analysis

Source: reference video R1 (see `evidence-log.md`). Observation IDs (O#) refer to that log.

## Workspace layout (O6, O7, O8, O18, O20)

| Region | Reference behaviour | Animara implementation | Status |
|---|---|---|---|
| Menu bar | File, Edit, View, Insert, Modify, Text, Commands, Control, Debug, Window, Help (O2) | Same order, `menus.rs` | Implemented |
| Workspace switcher | "Essentials" combo + publish/test buttons top-right (O6) | Combo with saved workspaces, Publish and Test buttons | Implemented |
| Tools | Single column on the left, grouped, colour chips at bottom (O7) | Same, 23 tools, original icons drawn procedurally | Implemented |
| Stage | Centre, edit bar with scene breadcrumb, scene/symbol pickers, zoom combo (O6, O12) | Same, plus rulers/grid/guides | Implemented |
| Timeline | Bottom dock with Output tab (O6) | Bottom dock: Timeline, Output, Motion Editor, Actions | Implemented |
| Right panels | Assets, Properties, Color, Align, Library (O6) | Same tabs plus Swatches, Transform, History, Info, Scene, Components | Implemented |
| Properties sub-tabs | Tool / Object / Frame / Doc (O18) | Same four tabs | Implemented |
| Docking | Panels are tabbed, dockable, floatable | `egui_stack` dock area: drag tabs to split/float | Implemented |
| Multiple windows/monitors | Duplicate Window (O19) | Panels can float as in-app windows; separate OS windows are not implemented | Partially compatible |

## Dialogs

| Dialog | Reference (O#) | Animara |
|---|---|---|
| New Document | Category tabs, preset grid, Width/Height/Units/Frame rate/Platform, Create (O3) | Same structure; platform fixed to Animara scripting |
| New from Template | Category list + template list + preview (O4) | Covered by New Document categories (Ads presets) + built-in sample files; no separate template browser |
| Document settings | Doc tab: W/H, Match contents, Scale content, stage colour, FPS (O18) | Doc tab and Modify ▸ Document dialog with the same fields |
| Welcome / sample files | Sample files strip in New Document (O3) | Welcome screen with recent files and 5 procedurally generated samples |

## Interaction behaviours

| Behaviour | Reference | Animara | Class |
|---|---|---|---|
| Selection outline with transformation point | O11 | Blue bbox, registration cross, transform point circle (Free Transform) | Observed → Implemented |
| Double-click instance → edit in place, other content dimmed | O12 | Same; parent drawn at 35 % alpha; breadcrumb | Observed → Implemented |
| Breadcrumb navigation back to scene | O13 | Clickable breadcrumb + back button + double-click empty stage | Observed → Implemented |
| Drawing new frame-by-frame strokes on later frames | O14 | Drawing on a frame beyond a layer's end inserts a keyframe | Observed → Implemented |
| Playhead scrubbing with frame counter and seconds ruler | O10 | Ruler scrub, frame counter, "1s/2s" markers | Observed → Implemented |
| Play/Pause icon toggles while playing | O16 | Play/Stop icon toggles | Observed → Implemented |
| Keyboard shortcuts in menus | O5, O19 | Shown in menus; customisable in Edit ▸ Keyboard Shortcuts | Observed → Implemented |
