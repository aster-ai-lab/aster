# Graphite icon generator repair

Base: Aster main `29650ead2`. The current tenant brand contract and committed SVG drawings are the source of truth.

## Result

Updated `scripts/app-icon.py` to reproduce the approved graphite plate, silver bevels, obsidian core and wrapped orbit at all three existing detail levels. Removed the retired blue constants. Generated the SVG files through the existing script; no SVG was hand edited.

Reopened all three results and compared their canonical XML with main's original drawings. Every element, its order, geometry, color, gradient, transform and descriptive text matches. Full and micro files are byte-identical to main; small differs only in whitespace. Filenames, 1024-pixel canvases, size bands and geometry parameters are retained. `icon-source-comparison.json` records original, generated and canonical hashes.

Added a regression against the actual generator output requiring its literal RGB colors to stay neutral, as required by `apps/web/public/tenant/aster/BRAND.md`. This prevents a stale generator from restoring the blue brand while passing freshness checks.

## Verification

- Before: the new palette guard and existing source freshness check fail (2 failures among 11 tests).
- `python3 scripts/app-icon.py` regenerates the three existing source files.
- `python3 tests/app_icon_assets.py`: 11 passed, including the existing source, raster, container and desktop identity checks.
- `git diff --check`: passed.

Main CI's Linux and macOS checks failed at the old icon freshness gate in [run 41017554651](https://github.com/aster-ai-lab/aster/actions/runs/41017554651). This repair addresses that observed gate without reverting the approved artwork. PR #208 must use the repaired source instead of its old-blue SVG changes.

## Explicitly not claimed

No new icon design, raster change, installer, release, full CI pass, or approval of the other PR #208 changes is claimed. Existing SVG identifiers and unrelated drawing behavior are preserved.
