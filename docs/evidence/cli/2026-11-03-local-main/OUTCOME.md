# Local main integration — 2026-11-03

## Result

The user authorized committing this task's implementation to local `main` and
persisting the unfinished roadmap. The current checkouts were already on `main`;
no extra worktree, remote push, release tag or publication was needed.

The remaining scope, ordered M0–M8 work, owners, exit conditions, six CLI timeout
cases and native/release prerequisites are persisted in
[ROADMAP.md](../../../../specs/031-cli-mise-replacement/ROADMAP.md).
The frozen 24 families, original 35 + 14 identities and T004–T014 remain required.

## Integration scope

- The plugins submodule commits domain-organized trusted AIX source units,
  acquisition recipes and corresponding tests. The unrelated Review source
  working changes remain outside this commit.
- The private Studio submodule commits tenant job persistence/retry, requested
  plugin/model/window/terminal/sidebar changes and performance-panel removal.
  A concurrent first-run gate was preserved in a follow-up commit; its dialog
  and tests were compared byte-for-byte with the concurrent commit afterward.
- Parent commits include the tool owner, environment/package-command callers,
  frozen references, scoped Pager lint repairs, packaging callers, documentation,
  implementation evidence and the two child repository references. Independent
  gateway/Core/TUI/release changes are retained outside this task's staging.
- Private Studio implementation snapshots are not added to the parent repository.
  Their hashes and execution records remain available; canonical implementation
  lives only in the private submodule.
- The shared working Cargo.lock is preserved. Cargo generated the selected
  dependency lock from an isolated snapshot of the staged manifests, and full
  offline metadata resolution was repeated with `--locked`.

## Verification

- `python3.13 -m unittest discover -s tests` in `plugins`: **12 passed**.
- Existing `scripts/test_runtime_components_defer.py` in Studio: the selected
  native packaging test failed before the fix because the publisher expected
  `plugins/operations/aster/<name>` while the producer staged the retired path.
  The producer now uses the admitted domain. All **6 packaging tests passed**
  afterward, using the real native AIX builder and fixture component trees.
- `cargo metadata --offline --format-version 1 --manifest-path <staged snapshot>`
  generated the selected lock; the same full resolution with `--locked` passed.
  This proves dependency resolution, not compilation or native runtime behavior.
- Staged path/whitespace, frozen-input, private-source exclusion, child ancestry
  and roadmap/reference checks are recorded in `integration.json`.
  Byte-identified upstream notices and the Node public key retain their original
  bytes; they are not reformatted to satisfy whitespace checks.

Historical 150 tool / 23 environment Pager / 18 native plugin / 77 renderer
witnesses and earlier failures remain in the original qualification packets.
Those dirty-checkout witnesses are not relabeled as an all-green immutable-commit
test run. The full native CLI/GUI/platform/release blockers are retained in the
roadmap; full compilation/tests were not repeated for this local commit operation.

## Explicitly not claimed

No full mise replacement, 49/49 migration, full test-suite pass, cross-platform
certification, final-archive/sandbox/signature/notarization qualification or
release publication is claimed. Local integration does not close those gates.
