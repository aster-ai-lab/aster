# Qualification cache retention

The existing qualification build already shares one Cargo target, sets `CARGO_INCREMENTAL=0`, and uses APFS clonefile for frozen members. No production build profile, candidate identity check, source freeze, or evidence contract was changed.

Added `scripts/aster_qualification_cleanup.py` for the missing retention step. It considers direct `fallback-state-*`, `deloop-changing-feed-*`, `deloop-independent-records-*`, `glm-tui-*`, `tui-*`, `rust-multifile-real-diagnostic-*`, `native-multiturn-diagnostic-*`, and `rust-std-multiturn-diagnostic-*` directories, keeps the newest two per family plus explicit `--keep` names, always preserves `tui-current-*`, and ignores shared dependency caches, evidence, symlinks, and other targets. Dry-run is the default. `--execute` refuses deletion when a candidate name appears in active process command lines.

## Verification

- `python3.13 tests/aster_qualification_cleanup.py`: 7 passed, 0 failed. Covers latest-two retention per family, current TUI asset protection, diagnostic candidate retention, explicit protection, real deletion through the CLI entry point, active-process refusal, dry-run preservation, and non-candidate preservation.
- `python3.13 -m py_compile scripts/aster_qualification_cleanup.py tests/aster_qualification_cleanup.py`: exit 0.
- `git diff --check -- scripts/aster_qualification_cleanup.py tests/aster_qualification_cleanup.py`: exit 0.
- `python3.13 scripts/aster_qualification_cleanup.py`: dry-run kept 2 newest candidates, listed 16 older candidates, and reported 0 active candidates. No directory was deleted by the preview.
- `python3.13 scripts/aster_qualification_cleanup.py --execute`: deleted those 16 candidates. Available space increased from 93,371,016 to 109,678,812 KiB, releasing 15.55 GiB before rebuilding.
- `CARGO_INCREMENTAL=0 cargo build --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.dev.package.sha2.opt-level=3 -p aster -p aster-acp --bins`: the first attempt reached the 300-second tool deadline during compilation; the resumed command completed successfully in 88.733 seconds with existing warnings.
- `aster --version` reports 0.1.8. CLI and ACP `--build-id` both report `aster-cli-pair-v1:37132f38c0d9727a7f3ea82db40b2d7215ec3e7a:aarch64-apple-darwin`.
- Final available space is 105,675,444 KiB, a net increase of 11.73 GiB over the pre-cleanup measurement. `target/qualification` is 301 GiB after rebuilding.

## Studio incremental cleanup and recovery

- `target/debug/incremental` was actively used and was preserved. The process listing showed no active `target/studio/aster` consumer before its 92 GiB incremental directory was removed. A second guard used an invalid macOS `pgrep` expression and did not itself establish absence; the prior and post-cleanup process listings are the actual observations.
- Available space increased from 105,989,508 to 176,611,936 KiB immediately after Studio incremental cleanup, releasing 67.34 GiB.
- `CARGO_INCREMENTAL=1 cargo check --locked --offline --target-dir target/studio/aster -p aster-studio --lib --no-default-features --features tenant-aster`: early attempts reached tool deadlines while recompiling dependencies; verbose output confirmed progress, and the resumed check completed successfully in 4 minutes 28 seconds with existing warnings.
- Final Studio incremental size is 1.3 GiB. Final available space is 171,186,816 KiB, a net increase of 62.17 GiB over the pre-incremental-cleanup measurement. The active default incremental directory remains and is 29 GiB.

## Deloop candidate retention

- The default incremental directory remained active and was preserved. The retention tool kept the newest two directories in each supported family and removed 46 older `deloop-changing-feed-*` and `deloop-independent-records-*` build directories with no active references. Shared caches and all other directories were preserved.
- Available space increased from 165,013,812 to 218,731,672 KiB immediately after deletion, releasing 51.22 GiB.
- Subsequent CLI/ACP builds repeatedly restarted compilation as other sessions changed source and encountered tool deadlines or a shared-target lock. No successful rebuild is claimed for this specific cleanup round.
- Final available space is 210,817,672 KiB and `target/qualification` is 248 GiB. This is a net increase of 43.68 GiB over the pre-deloop-cleanup measurement and 112.00 GiB over the initial measurement before any cleanup.

## TUI and GLM candidate retention

- Removed 46 older `glm-tui-*` and `tui-*` build directories with no active references. Kept the newest two per family, explicitly kept the previous Root 6b candidate, and preserved both `tui-current-runtime-c78de45d` and `tui-current-compiler-runtime-c78de45d`.
- Available space increased from 210,303,612 to 255,151,848 KiB, releasing 42.76 GiB. `target/qualification` is now 205 GiB.
- A repeated dry-run reports no remaining expired candidates in the supported families. No additional Cargo rebuild was started for this cleanup-only round.

## Diagnostic candidate retention

- Removed 34 older `rust-multifile-real-diagnostic-*` and 4 older `native-multiturn-diagnostic-*` build directories with no active references, keeping the newest two per family. A newly arrived deloop candidate also caused one older `deloop-changing-feed-*` directory to expire under the same retention rule; 39 directories were removed in total.
- Available space increased from 254,022,772 to 298,994,156 KiB, releasing 42.88 GiB. `target/qualification` is now 161 GiB.
- A repeated dry-run with the explicit historical protections reports no remaining expired candidates. No Cargo rebuild was started for this cleanup-only round.

## Completed experiment cleanup

- Removed 26 explicitly named completed-experiment directories covering compaction, summary, batch history, capacity, startup, artifact search, static contracts, and early code-profile work. Each path was checked for active processes immediately before deletion. Evidence directories, the shared capacity target, current candidates, the `r16` source tree, and current TUI assets were preserved.
- Available space increased from 296,949,256 to 322,269,448 KiB, releasing 24.15 GiB. `target/qualification` is now 135 GiB.
- The remaining qualification content is the shared capacity target, current/protected candidates, current TUI assets, the preserved `r16` tree, and small supporting artifacts. No Cargo rebuild was started for this cleanup-only round.

## Shared Core and default incremental cleanup

- `cargo clean --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 -p aster-core` removed 8,757 files and 16.6 GiB. The default incremental directory had no active compiler at that moment and was removed; the running Studio target was preserved.
- Available space increased from 320,202,560 to 363,588,624 KiB immediately after these cleanups, releasing 41.38 GiB.
- Repeated CLI/ACP build commands reached tool deadlines during recompilation or shared-target locking. No successful build is claimed from those commands. The currently present binaries were later executed successfully: `aster --version` reports 0.1.8, and CLI/ACP both report build identity `aster-cli-pair-v1:f45e03b0eb32abec36e5f5bbcfb3f2a148ae93c7:aarch64-apple-darwin`. Their writer is not attributed.
- Final available space is 358,935,728 KiB, a net increase of 36.94 GiB over the pre-cleanup measurement and 253.26 GiB over the initial measurement. `target/qualification` is 120 GiB, its shared capacity target is 99 GiB, and the regenerated default incremental directory is 216 MiB.

## Durable build policy

- Development keeps the existing shared debug target and incremental compilation. Qualification keeps its existing shared target and `CARGO_INCREMENTAL=0`. Production keeps the independent release profile; no release compiler flags were changed.
- The active qualification `build.py` now invokes `scripts/aster_qualification_cleanup.py --execute` only after a successful source freeze. Retention failure fails the build rather than silently leaving unbounded candidate growth.
- `python3.13 -m py_compile docs/evidence/code/2026-11-02-summary-input-retention/build.py scripts/aster_qualification_cleanup.py tests/aster_qualification_cleanup.py`: exit 0. Retention tests: 7 passed. `python3.13 tests/code_release_workflow.py`: 6 passed.

## Explicitly not claimed

- No release-profile binary size or runtime performance improvement is claimed; this was a shared-cache development-profile rebuild.
- No full target cleanup, protocol test, PTY test, real-model task, final release qualification, or end-to-end execution of the new post-freeze retention hook was performed; the hook was verified by syntax checks and its called tool's hermetic tests.
- The process check is conservative and requires `pgrep` for execution; platforms without it fail explicitly rather than deleting without the check.
