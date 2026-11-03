# Studio plugin and chrome follow-up — 2026-11-03

The obsolete project/task Wiki selection grid and both group-level More buttons are removed. The old eight-project recent slice and its now-unused pin bookkeeping are also removed: all loaded project records remain accessible in source order in the existing sidebar scroll area. Object actions still open their authoritative scoped Wiki directly; the scope binding, dirty editor guard and focus return are preserved. The deleted grid's CSS, catalogue reads, pagination and unused copy are removed. DESIGN.md now records this behavior.

The ActivityBar remains 60px with no added left content gutter. The macOS host places compact native controls inside the measured rail, including webview zoom, and reapplies placement on resize, display scale/focus changes, page load and overlay restoration. Geometry cases cover 0–60 point rails and enlarged input sizes. The renderer reports actual rail/viewport width to the existing native host.

First-task model picking retains declared reasoning choices through the Codex inventory, local model mapping and picker. Unsupported host tokens are excluded at the native inventory boundary; names do not infer effort vocabularies. The composer falls back to its chosen model's Provider logo before ACP exists and does not substitute a different ACP current model for an explicit unbound selection.

Terminal sessions share the workspace's single tab strip. New, selection, close, close-others/right/all and retained hidden PTYs use the existing workspace APIs. The redundant inner terminal tab header is removed.

Performance-panel entry points, Profiler, navigation/render metrics store, reserved panel height and feature-only tests are deleted. Native runtime packages are tagged by the host's exact owned identities, and both plugin views hide them. The installation dialog retains manual optional-plugin install/retry. Search copy is 搜索关键词或插件名.

Runtime installation no longer treats a loose Office companion without all AIX metadata as a bundled package. Runtime jobs publish checking/resolving/downloading/verifying/installing stages and observed retained bytes from TrustedRegistry. Metadata-only cache misses retry the trusted pinned archive; changed archive pins still fail. Inspection reports receipt and runtime-file checks. Progress remains visible with details collapsed and before the first list response; failed optional installs can retry without treating failed removals as installs.

## Verification

From repository root, pinned Rust 1.97.1, macOS arm64:

- `CARGO_TARGET_DIR=target/studio/aster cargo test --locked --offline -p aster-studio --lib --no-default-features --features tenant-aster -- traffic_lights_tests capabilities::registry::tests runtime_components:: codex_inventory_tests --test-threads=1`: **22 passed, 0 failed**. Includes a real locally signed TUF/AIX download fixture, offline reuse, changed-pin rejection, incomplete Office bundle metadata and optional retry coverage.
- `CARGO_TARGET_DIR=target/studio/aster cargo clippy --locked --offline -p aster-studio --all-targets --no-deps --no-default-features --features tenant-aster`: exit 0; unrelated existing warnings remain in the retained log. New runtime-event needless borrow and Codex nested-if candidates were corrected.
- `CARGO_TARGET_DIR=target/studio/aster cargo build --locked --offline -p aster-studio --bin Studio --no-default-features --features tenant-aster`: exit 0, debug build.
- `cargo fmt --all -- --check`: exit 1 only for the existing unformatted `apps/studio/native/data-sqlite/src/main.rs`; affected Rust sources were formatted with pinned rustfmt. That unrelated file was not rewritten.

From `apps/studio`, using the installed repository tools:

- `node node_modules/vitest/vitest.mjs run --project renderer` with the 14 affected suite paths in `renderer-combined.log`: **304 passed, 2 failed** out of 306. The two TabRouter cases fail identically with the saved pre-change TabRouter: **9 passed, 2 failed**. The before/after baseline changes only imports to the saved implementation/dependency; production source was never reverted and no assertion was changed to obtain a pass. These failures concern explicit visible aria-hidden=false and the pre-120ms loader expectation, outside performance-instrumentation removal.
- `node node_modules/@rsbuild/core/bin/rsbuild.js build`: exit 0, 3.17s; the removed performance-component import error is gone.
- Scoped installed Oxlint with `--deny-warnings`: exit 0. Scoped ESLint: exit 0, 15 existing/ignored-file warnings. Scoped Oxfmt: exit 0, 31 files. Source/deletion references and `git diff --check` are retained.
- `node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false`: exit 2, **285 errors outside changed sources**. The earlier default-heap attempt exhausted memory; it is not counted as a successful check.
- The sidebar-cap regression failed before the implementation (1 failed, 105 skipped), then the complete WorkbenchHome suite passed (106 passed). This rerun is a subset of the 304 passing combined tests, not an additional 106 independent cases. Four existing fixture typing errors in that touched suite were corrected with owned PNG byte arrays and the already-asserted DOM node; no expected value was relaxed. The final 8GiB typecheck is retained separately from the earlier 289-error log.
- Existing i18n sync/check: exit 0, **25,532 translations checked**.

The SHA-256 values in qualification.json identify reviewed files in a concurrently edited dirty checkout; they do not qualify an immutable release commit. Private Studio implementation snapshots are retained outside parent tracking; the implementation is committed only in its private submodule. Parent-owned source copies, execution logs and identity records remain in this packet. Reopen/hash comparison verified the retained snapshots and deleted paths before local integration.

## Actual pinned Office package

The rebuilt `target/debug/aster-use` (0.3.4) added `aster-ai-lab/plugins` with the running app's exact pinned root SHA in an isolated `ASTER_USE_HOME`. Its install plan matched package ID, complete build version, target, archive length/SHA-256, expanded-package digest and manifest digest against the app's Office pin. The reviewed plan had an empty permission ceiling, no secret changes and no workspace impacts. Applying that exact operation/digest succeeded; list-installed reported `observed: ready` at the same version/digests. This was a real signed TUF v9 catalogue and GHCR AIX download, not the loopback fixture.

The installed executable then passed **8 actual commands**: version; create DOCX/XLSX/PPTX; edit DOCX; read back DOCX/XLSX/PPTX. All three resulting OOXML files were reopened with ZIP CRC and expected XML-part checks; the persisted DOCX text and CLI readback matched the actual edit. Files used a path containing spaces and non-ASCII characters. Commands, expected pin, reviewed plan, apply/list results, executable digest and generated fixture hashes are retained under `office-actual/`. No chmod, re-signing, metadata injection, fallback executable or user-profile mutation was used to make the package pass. This qualifies the actual release package and canonical SDK installation; native Studio IPC/GUI remains unexecuted.

## Explicitly not claimed

- No successful native GUI run: the available computer-use connection returned native-pipe startup failure and unavailable browser host. No screenshot was invented or substituted.
- No installation in the user's actual profile or native Studio GUI end-to-end run. The exact Office release package passed isolated installation and executable checks; this does not qualify every plugin/upstream release artifact.
- No restart or replacement of the user's running desktop. Native chrome requires the newly built host on a subsequent launch; renderer hot reload alone cannot replace its native implementation.
- No Windows/Linux native runtime qualification, all-green workspace/type gate, CLI/ACP/sandbox release smoke, signed/notarized archive, channel manifest, commit, tag, upload or release. The broad mise-replacement goal remains active.
