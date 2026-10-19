# T0.3.1 — three-lineage shared-crate drift ratchet

| Field | Value |
| --- | --- |
| Date | 2026-10-19 |
| Task | A3S OS convergence program WS0.3 / T0.3.1 (program doc: `apps/cloud/docs/convergence/plan.md`) |
| Landed in this repository | `tests/crate_drift_report.py`, `tests/crate_drift_baseline.json`, CI step in `.github/workflows/ci.yml` |

## Result

Landed. The three crate lineages (aster `crates/`, `apps/api/crates`,
`apps/cloud/crates`) now have a measured, shrink-only drift baseline and a
ratchet that fails when drift grows.

## Baseline (2026-10-19)

- Lineage sizes: aster=180, api=14, cloud=64 crate names.
- Shared names (in ≥2 lineages): 62. Drifted: 11. Unique: 125.
- Drift inventory (the ratchet set):
  - CONTENT-DRIFT (same version, different bytes): `a3s-auth`,
    `a3s-code-core`, `a3s-effect`, `a3s-flow`, `a3s-form-core`, `a3s-orm`,
    `devices`, `vm-memory`
  - DUPLICATE-aster: `a3s-oci-sdk`
  - DUPLICATE-cloud: `a3s-libkrun-sys`, `libkrunfw-windows`
- Notably NOT drifted today (byte-identical copies): the rest of the
  shared family, including `a3s-acl`, `a3s-boot`, `a3s-event`, `a3s-lane`,
  `a3s-sms`.

## Commands and counts

- `python3.13 -m unittest tests.crate_drift_report` — 4 tests, OK
  (same-content, content/version/unique classification, within-lineage
  duplicate, baseline shrink-only ratchet).
- `python3.13 tests/crate_drift_report.py --update-baseline
  tests/crate_drift_baseline.json` — baseline written.
- `python3.13 tests/crate_drift_report.py --baseline
  tests/crate_drift_baseline.json` — exit 0 (no growth).
- Missing-lineage probe: `build_report` with an absent lineage root exits
  fail-closed ("lineage roots missing").
- `python3 -c "import yaml; yaml.safe_load(...)"` — ci.yml parses.
- CI step "Shared-crate drift ratchet" added after the internal
  dependency boundary step, conditioned on both submodule crate trees
  being present (`hashFiles`), so it enforces wherever submodules are
  checked out and is visibly skipped otherwise.

## Defects found and fixed during development

1. `tomllib` top-level vs `[package]` table — the first version read
   `name`/`version` from the root table, collected zero crates, and the
   same-content test passed vacuously. Fixed the parser and hardened the
   test to assert collection, not just classification.
2. Within-lineage duplicate names (`a3s-libkrun-sys` exists twice under
   aster `crates/`, and under cloud `crates/` alongside
   `libkrunfw-windows`) aborted the whole report; now recorded as
   `DUPLICATE-<lineage>` findings inside the ratchet.

## Explicitly not claimed

- No drift was fixed; the 11-entry set is recorded, not resolved.
  Resolving it is DP-3 (single upstream) work in Phase 3.
- Hosted aster CI has not run: the submodule SSH URLs mean the ratchet
  step is expected to skip there until submodule credentials or a
  self-hosted runner exist; the condition is explicit, not silent.
- DP-3 (single-upstream mechanism) is not decided; due 2026-12-18.
