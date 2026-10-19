# Path regimes for the three Cargo workspaces

## Result

The constitution now states a product regime and a control-plane regime.
Product paths keep hermetic tests, Tauri commands, and the root
workspace. Control-plane paths name bounded contexts, contiguous cloud
migrations, `#[controller]`, and dual-adapter proofs. `apps/api` and
`apps/cloud` stay out of the root member list and keep their own
workspaces.

The 2026-10-18 migration record still says this repository had no
`apps/cloud` layout. That file was left as written. This record is the
correction.

## What landed

- `.specify/memory/constitution.md` — Regimes table; principles III, V,
  VI, and X follow the path.
- `docs/development-standards.md` — its own Product regime and
  Control-plane regime sections, plus §1, §3, §6, §7, and §8.
- `AGENTS.md` — one paragraph pointing at the table.
- `apps/cloud/AGENTS.md` — one line pointing at the control-plane regime.
- `docs/spec-driven-development.md` — an epic's constitution check uses
  the regime of its paths.
- `tests/governance_regime.py` — reads both documents and the three
  manifests. An in-memory copy of each document fails the same check
  when it restores an exclusion sentence, puts `CLOUD_MIGRATION_COUNT`
  in the product regime, or drops a control-plane token.

## Commands and counts

- `python3 -m unittest tests.governance_regime -v` — 4 tests, OK.
  The local `python3` is 3.9.6, so the test reads the member list
  without `tomllib`. The tests cover both documents, a restored
  exclusion sentence on each document, an obligation regression, and the api/cloud
  workspace split.
- `python3.13 tests/crate_drift_report.py --baseline tests/crate_drift_baseline.json`
  — exit 0. Lineages aster=180, api=14, cloud=64. Shared 62, drifted 11,
  unique 125. The baseline file was not rewritten.

## Explicitly not claimed

No workspace was merged. The drift baseline was not changed.
`apps/cloud/crates/control-plane` was not registered in the root
workspace. No crate bytes were reconciled. DP-1 through DP-5 were not
reopened. The sparse index was not created.
