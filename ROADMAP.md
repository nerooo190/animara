# Roadmap

Milestones follow the phases of the original plan. Items already delivered are listed in
[FEATURE_PARITY.md](FEATURE_PARITY.md).

## Next

1. **Reference validation with real files.** Run the FLA/XFL importer against Animate-authored documents, record
   differences in `docs/reverse-engineering/format-compatibility.md`, and add the files as fixtures where licensing
   allows.
2. **Merge-drawing mode.** Edge-based shapes that merge and cut on contact, plus gap-closing paint bucket fills.
3. **GPU renderer** (Vello/wgpu) for the stage, keeping tiny-skia as the reference and export path. Target: 60 fps
   for the 1 000-element benchmark.
4. **Native audio** (cpal) with audio scrubbing and sample-accurate sync in the editor.
5. **Motion-tween property keyframes** inside spans, and graph editing per property.
6. **Rich-text editing** of individual runs.
7. **Bitmap mesh deformation** and warp tweens for asset warp.
8. **PSD and Lottie import**, plus SWF import (shapes and timelines).
9. **Multiple OS windows** (egui viewports) for panels and Duplicate Window.
10. **Script debugger** with breakpoints and variable inspection.
11. **Accessibility:** keyboard navigation of the stage and screen-reader labels (AccessKit).
