# Aster-vec sparse delta keys and catalog admission

## Root causes and changes

The incremental generation planner allocated appended engine keys from
`previous_chunks.len()`. After an edit created sparse keys such as `d0,d2,d3`,
a later remove-plus-append allocated `d3` again. The full-build fallback hid
this from the older functional test while logging
`aster-vec delta produced a duplicate appended primary key`.

The new regression failed before the fix with:

```text
appended keys ["d3"] collide with live keys ["d0", "d2", "d3"]
```

The planner now allocates keys not present in the live previous set. The
writer consumes those exact caller-allocated keys instead of independently
regenerating them. It validates key count, non-empty/NUL-free keys, and
uniqueness before applying the delta. Existing edit, rename, reopen, oversized
churn, and missing-source fallback behavior remains covered.

The historical catalog log contained about 80,000 failed files but did not
retain their individual messages, so it is not evidence for one unique cause.
The code did admit generated `docs/evidence` artifacts, allowing them to
consume file/chunk/text/index budgets ahead of source files. These artifacts
are now excluded from the retrieval catalog (not from ordinary grep/file
access). A controlled reconciliation test proves they are neither read nor
allowed to consume a one-file source budget. Partial-reconciliation logs now
include at most three failure samples for future diagnosis.

## Verification

Environment: Darwin arm64, rustc 1.97.1.

- Full workspace retrieval suite: **188 passed, 0 failed**, including the new
  sparse-key, generated-evidence, and typed-failure-count regressions.
- Persistent-index subset: **13 passed, 0 failed**.
- ACP incident integration test passed in both default modes: interactive
  **868 ms**, unattended **1,615 ms** for the whole mocked prompt.
- Existing unattended BM25/grep ACP protocol test passed.
- `cargo clippy --locked -p aster-core --all-targets --no-deps`: exit 0, with
  existing warnings only.
- `cargo clippy --locked -p aster-acp --all-targets --no-deps`: exit 0, with
  existing warnings only.
- Scoped rustfmt and `git diff --check`: passed. An earlier full
  `cargo fmt --all -- --check` passed; the final repeat reported two unrelated
  existing diffs in `apps/studio/src-tauri/src/providers_local.rs`, which were
  left untouched.

Primary commands:

```bash
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-core --lib workspace::retrieval::

CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test unattended_e2e \
  incident_grep_is_bounded_through_default_interactive_and_unattended_acp \
  -- --exact --nocapture --test-threads=1

CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo clippy --locked -p aster-core --all-targets --no-deps

CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo clippy --locked -p aster-acp --all-targets --no-deps
```

## Real tasks

The ACP binary was built from the current sources with the same default
features in `target/grep-budget-tests` and copied to `target/debug/aster-acp`.

- A short retrieval-enabled probe showed no duplicate-key, partial-catalog, or
  reconciliation warning. It was not a full rebuild.
- Immediate incident grep: exit 0 in **28.168 seconds**.
- Full repository-analysis task: exit 0 in **47.556 seconds**.
- `docs/evidence/code/2026-11-04-grep-e2e/debug_immediate.py` now supports
  retrieval-enabled logging and does not kill an already exited process group.

## Isolated full rebuild and remaining failures

`crates/code/acp/tests/full_index_rebuild.rs` is an opt-in live qualification
test. It uses a fresh temporary state root, the real embedded light-tier model,
no model requests, and a 240-second readiness ceiling. The semantic queue was
allowed to drain before querying. Run command:

```bash
RUST_LOG=info,aster_core::workspace=debug \
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test full_index_rebuild \
  -- --ignored --nocapture
```

The default-budget rebuild did **not** reach Ready. Final observed status after
**155.697 seconds**:

- Eligible files: **44,399**; indexed files: **4,310**.
- Indexed chunks / vector records: **18,103**; coverage: **970 bps (9.70%)**.
- Failed files: **40,089**, consisting of **40,080** `index byte estimate`
  budget failures (64 MiB default) and **9** `TooManyChunks` failures (128
  chunks per file).
- Semantic `GREP_SCAN_TIMEOUT` query returned **3 hits**, none under
  `docs/evidence`. The partial index is usable, but incomplete.
- No duplicate appended primary-key warning appeared.
- The live test then failed its Ready/zero-failure assertions, as required;
  this is retained as an unmet result, not reported as a pass.

The typed failure classification is now emitted as `failure_kinds` alongside
three samples, without parsing rendered error strings. Raw output is in
`rebuild.log`.

## Explicitly not claimed

- No claim that every historical 80,000-file failure had the same cause; the
  old log lacks per-file errors. The latest rebuild establishes the two
  current classes above instead.
- No default-budget Ready or complete-index claim is made. Production default
  memory/chunk limits were not enlarged to force a green result.
- No claim that generated `docs/evidence` files remain searchable through the
  retrieval catalog. Their exclusion is intentional; ordinary grep and file
  access are unchanged.
- No Windows or Linux runtime verification was performed.
- No full default-target `just build` success is claimed for this final change;
  the verified ACP test-target binary was used for the real tasks.
