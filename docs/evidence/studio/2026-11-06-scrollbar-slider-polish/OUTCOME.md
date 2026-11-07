# Quiet scrollbar and reasoning slider polish

Verified on 2026-11-06, macOS arm64. No commit was created.

## Result

- The model-detail scroller keeps its quiet scrollbar visible during scrolling
  and hides it after 180 ms idle. Thumb color transitions over 120 ms instead
  of appearing abruptly.
- The detail body uses `overscroll-contain` and `scrollbar-gutter: stable` to
  avoid parent scroll chaining and layout shifts.
- The reasoning slider keeps its centered-thumb fix while increasing the
  interaction height to 40 px and thumb size to 20 px.
- The thumb gains a visible border, pointer/grabbing cursors, hover feedback,
  and a 2 px keyboard-focus ring with popover offset.
- Track color, tick positions, effort options, default range behavior, and
  wheel/keyboard selection behavior are unchanged.

## Verification

- Focused detail-card and slider tests: **2 files, 33/33 passed**.
- Full model-selector, Composer, and naming regression suite:
  **9 files, 100/100 passed**.
- Scoped Oxfmt passed. Scoped Oxlint reported **0 warnings and 0 errors**.
- Production Rsbuild output `.studio/dist/scrollbar-slider-polish`: passed.
- `STUDIO_RENDERER_DIST=.studio/dist/scrollbar-slider-polish just build-studio`:
  passed with full companion staging.
- Production CSS confirms:
  - `.h-10 { height: calc(var(--spacing) * 10) }`
  - the slider thumb is `calc(var(--spacing) * 5)` in both dimensions;
  - 2 px focus ring and popover offset are present;
  - `data-scrolling=true` and `scrollbar-gutter: stable` are present.
- `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- No real-window screenshot or measured geometry acceptance is claimed. The
  available Aster window was covered by a workspace dialog during the final
  visual check.
- No change to the standard 44 px WCAG target is claimed; this compact control
  uses a 40 px desktop interaction height and a 20 px visible thumb.
- No full frontend suite, full TypeScript check, Windows/Linux GUI run, release
  installer, or full-repository lint result is claimed.
