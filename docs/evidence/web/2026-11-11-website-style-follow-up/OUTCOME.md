# Website style follow-up

## Result

Restored the palette, card styling, and download gradients from the
pre-Hallmark stylesheet snapshot. Kept the current copy, nine original
product images, the default TUI startup panel, and the click-to-switch Studio
steps. Preserved the local adjustments for focus, contrast, touch targets,
and narrow viewports. The download script uses its original `enhanced` hook.

## Verification

On macOS arm64, these commands exited 0.

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web run check
npx --yes --package=node@24.12.0 -- node --test apps/web/tests/home.test.cjs apps/web/tests/download.test.cjs
git diff --check
```

- Astro built 223 pages. The site checker passed for 276 files, 9 home tabs,
  219 documentation pages, 2 legal pages, and 5 release URLs.
- 35 homepage and download tests passed, including the nine image identity
  checks. Zero failures or skips.
- The local preview on port 7100 served HTTP 200 for both pages and both
  stylesheets. The response bytes matched the fresh build.
- The local Chromium smoke script
  `target/web-checks/2026-11-11-style-follow-up-smoke.cjs` passed at 1440 × 900
  and 375 × 812. It checked the real TUI startup image on both pages, viewport
  fit, and independent TUI tab selection. Reviewed both TUI viewport captures.
  The first smoke invocation received a cached 304 on the second viewport.
  The successful run disabled the browser cache.

## Landing checks and cleanup

```sh
npx --yes --package=node@24.12.0 --package=npm@11.6.2 -- npm --prefix apps/web test
python3 apps/web/scripts/stage_site.py target/web-checks/2026-11-11-website-commit-site
git diff --check
```

All three commands exited 0. The full suite passed 22 Python and 135
JavaScript tests, with zero failures or skips. Public staging verified 276
allowlisted files. The test log and stage receipt remain under
`target/web-checks/` as `2026-11-11-website-commit-test.log` and
`2026-11-11-website-commit-stage.json`.

Stopped the Astro preview process (PID 56481) and confirmed port 7100 was
released. Removed 58 ignored paths (about 227 MiB) containing generated site
copies, build caches, reference downloads, and Hallmark scratch files. Kept
the public images and the recorded verification results.

## Explicitly not claimed

Physical phone or cross-browser verification, new native product captures,
or a deployment.
