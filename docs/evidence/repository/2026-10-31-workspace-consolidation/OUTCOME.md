# Workspace consolidation — upstream checkpoint

Consolidation is in progress. This record does not claim that every workspace is merged, that `work/` is removed, or that a release is qualified.

## Source checkpoint

The parent merge integrates upstream `19300db7df6fa799441fd455c5949b3f875176eb` with the preserved local source checkpoint. Current image input, session cancellation, memory management, permission modes, capture packaging and bounded background execution remain present. The shared background task registry retains its session tracker when a capable sandbox is attached.

The thirteen registered Studio source heads have been integrated. A later audit found thirty-four independent nested submodule clones omitted by Git worktree registration; all heads and five dirty source snapshots are preserved locally. Seven of those nested source heads and the remaining parent worktree heads still need integration. No new worktree was created.

## Unavailable Dashboard object

The incoming parent gitlink names `0a8c87a6373a23216ca63b944ba583e0ae8a7bf0`. Its configured remote `git@github.com:kense-labs/dashbaord.git` refuses that object, and the GitHub commit API reports it missing. The local registered and independent Dashboard checkouts do not contain it.

Dashboard `17723038069cffdadbdcd9bebcc5a1993e6bb9de` retains the available operations/login head `05e613a23601fc9cc370934602cf51fc969511a9` and reconstructs the documented shared Chart integration from canonical `packages/ui` sources. Nineteen copied source files were compared byte-for-byte. This recovers the documented behavior; it does not establish ancestry from, or exact equivalence to, the unavailable commit. The original shared-chart outcome remains unchanged.

## Verification actually completed

- `cargo check --locked -p aster -p a3s-code-acp -p a3s-studio`: passed after repairing the merged Codex home lookup and duplicate supplier-credential loading.
- Core background module: 29 tests passed, including session tracker retention after sandbox admission.
- Codex account importer: 11 tests passed.
- CLI release assembly: 7 tests passed; the macOS publisher now uses portable `shasum` while retaining both installed-archive sandbox gates.
- Shared chart: 12 tests passed. Initial missing ECharts dependency failures are retained; the pinned Bun 1.4.2 workspace install regenerated its lock before isolated and full reruns.
- Dashboard: 30 tests, typecheck, production build and CSS contract passed. Browser preview rendered a real chart canvas, ARIA label and long-range dataZoom without page errors. This used explicit synthetic preview data, not a production account.
- The merged FortuneSheet patch passed `git apply --check` and actual application against the published 1.0.4 package. Its declaration CRLF bytes were preserved. This is patch application proof, not full Office functional qualification.
- The JSON Render catalog was regenerated with the actual `a3s-form-core json-render-catalog` command. The website lock was regenerated with Node 24.12.0/npm 11.6.2, and the download snapshot was regenerated from published GitHub assets, currently macOS v0.1.7.

## Remaining gates

The Studio web typecheck still has unresolved diagnostics. Parent source merges, nested clone integration, shared popup/native GUI acceptance, native optimization acceptance, issue/PR work, workspace cleanup, final formatting and package qualification remain open. No new DMG or GitHub Release has been published from this checkpoint.

Historical evidence bytes retain their original whitespace. Source whitespace checks exclude those preserved raw records; they are not rewritten to manufacture a green check.
