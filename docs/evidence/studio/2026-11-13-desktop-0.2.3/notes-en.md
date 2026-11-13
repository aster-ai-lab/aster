## New Features

- None. This is a maintenance release.

## Improvements

- Locale week-start resolution now accepts both the finalized `Intl.Locale.prototype.getWeekInfo()` method and the `weekInfo` getter shipped by current V8 runtimes, with a documented Sunday fallback when neither is available.

## Bug Fixes

- Fixed the usage heatmap and distribution chart throwing `locale.getWeekInfo is not a function` on runtimes that expose `weekInfo` as a getter instead of the finalized method (verified on Node 22 / jsdom). The eight renderer assertions that failed for this reason now pass.
