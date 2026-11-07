# Reasoning-effort slider: centered thumb and continuous fill

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

- The 2026-11-06 `slider-thumb-center` fix never took effect in a browser. The
  shared Arc thumb reset (`top:auto; translate:none`) is emitted in `@layer
  app`, while the local `top-1/2` / `-translate-y-1/2` overrides are Tailwind
  `@layer utilities`; app outranks utilities. Arc also makes the thumb
  absolute, collapsing Radix's thumb wrapper to 0px. The measured thumb center
  sat 10px below the track center at every effort step.
- Both centering overrides now carry `!important`. A `-mr-2.5` on the range
  adds one thumb radius back so the fill stays under the knob instead of
  leaving a gap that widens toward the fastest effort.
- Changed `ModelSpeedControl.tsx` and its UI test. No shared primitive was
  edited.

## Verification

- Real Chromium layout measurement using the 2026-11-06 production CSS and a
  fresh build of this checkout, at effort 0/25/50/75/100%:
  - Before: thumb offset **+10px** at every step; fill-to-thumb gap **20, 15,
    10, 5, 0px**.
  - After: thumb offset **0px** at every step; gap **10, 5, 0, -5, -10px**,
    all within one thumb radius, so the knob covers the fill end.
- Fresh `STUDIO_RENDERER_DIST=.studio/dist/reasoning-slider-centering rsbuild
  build`: **passed** (51.3s). Production CSS contains `top:50%!important`,
  `translate:...!important`, and
  `[data-slot=slider-range]{margin-right:calc(var(--spacing) * -2.5)}`.
- Targeted Vitest (`ModelSpeedControl.ui.test.tsx`,
  `ModelSelectorDetailCard.test.tsx`): **33/33 passed**. Broader model-selector
  run: **8 files, 91/91 passed**.
- Scoped Oxfmt: **passed**. Scoped Oxlint `--deny-warnings`: **0 warnings, 0
  errors**. `git -C apps/studio diff --check`: **passed**.

## Explicitly not claimed

- No Studio native build (`just build-studio`), no live Aster window, and no
  GUI end-to-end scenario. The measurement loads the real production
  stylesheet against a faithful static replication of the Radix slider DOM,
  not the running React popover.
- No full frontend suite, full TypeScript check, Windows/Linux run, or commit.
- The shared `@aster-ai-lab/ui` slider primitive was not changed. Its
  app-layer thumb reset and the collapsed thumb wrapper still affect the other
  Slider call sites; promoting this fix upstream is a separate decision.
- No change to effort options, ticks, wheel/keyboard selection, or the
  provider-Default no-fill behavior.
