# Reasoning-effort slider thumb centering

Verified on 2026-11-06, macOS arm64. No commit was created.

## Result

- Restored `top-1/2` and `-translate-y-1/2` on the reasoning-effort slider thumb.
- These overrides counter the shared Arc thumb's `top:auto` and `translate:none`
  reset, so the thumb is centered on the track instead of below it.
- Tick positions, full-row layout, 32px interaction height, 8px track, default
  range behavior, and effort-selection behavior were not changed.

## Verification

1. Before the fix, the focused suite failed the new
   `centers the thumb on the track instead of below it` assertion: **1 failed,
   22 passed**.
2. After the fix, `ModelSpeedControl.ui.test.tsx` passed **23/23 tests**.
3. The combined model-selector, reasoning-slider, Composer, and model-naming
   regression run passed **9 files, 97/97 tests**.
4. Scoped Oxfmt passed. Scoped Oxlint with `--deny-warnings` reported
   **0 warnings and 0 errors**.
5. Production Rsbuild output `.studio/dist/slider-thumb-center` passed.
6. `STUDIO_RENDERER_DIST=.studio/dist/slider-thumb-center just build-studio`
   passed with full companion staging.
7. The production CSS contains both selectors:
   - `[data-slot=slider-thumb]{top:50%}`
   - `[data-slot=slider-thumb]{--tw-translate-y:calc(calc(1 / 2 * 100%) * -1);translate:var(--tw-translate-x) var(--tw-translate-y)}`
   It also retains the shared Arc reset these local rules override.
8. `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- No stable real-window geometry screenshot or measured `getBoundingClientRect`
  result is claimed. The default window's active task repeatedly closed the
  hover card, and the attempted fork reused that default instance rather than
  providing a separate usable window.
- No tick-position change, new default-fill behavior, full frontend suite, full
  TypeScript check, Windows/Linux GUI run, or release installer is claimed.
