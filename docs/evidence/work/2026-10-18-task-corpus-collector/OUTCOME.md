# Read-only collection for the desktop task corpus

Result: the collector and three small hermetic self-checks are ready. This is not
a desktop E2E result or a task-quality pass.

## Changes

- Added the ignored `collect_task_corpus_delivery` entry point in the Work test
  binary. It requires explicit state root, session UUID, assistant anchor UUID,
  case ID and output run root. A07 additionally requires an observed post-turn
  original-file SHA-256.
- Reads only the selected session's persisted message file, the host execution
  summary, and artifacts declared by that exact anchor's typed proof. It does
  not initialize the account/provider stores, submit HTTP/API requests, launch
  ACP, or inspect unrelated workspace files.
- Requires an authoritative terminal matching the selected assistant and proof
  identities. Collects only `type=text` assistant parts; reasoning, resume
  tokens, whole-message snapshots, account files and source attachments are not
  exported.
- Reuses `ArtifactStore::read` for the existing receipt, session binding, content
  digest and byte-count checks. Successful turns export only traced artifacts
  from that execution. Failed/cancelled turns export the real partial text and
  terminal metadata, without certifying partial files as completed deliveries.
- Writes a new `run/TC-ID` directory using create-new files, bounded to 20 MiB
  total. Filenames are `delivery-N-<source name>`; stdout records the original
  names and immutable hashes. Existing case directories are refused. A changed
  persistent turn, missing trace, unsettled part or corrupt snapshot refuses
  collection rather than scanning the workspace for a replacement.
- Added the invocation and collection constraints to the existing task-corpus
  `PLAN.md`. Screenshots and logs stay in a separate evidence directory.
- Added the two ordinary self-checks to the existing Work CI filter. The
  parameterized collector remains ignored and cannot execute in CI by default.

## Commands and results

```sh
cargo test --locked -p aster-dream --lib task_corpus_collect_tests -- --nocapture --test-threads=1
rustfmt --edition 2024 --check apps/work/src-tauri/src/agent_plane/task_corpus_collect_tests.rs
git diff --check -- apps/work/src-tauri/src/agent_plane/mod.rs apps/work/tests/e2e/task-corpus/PLAN.md
target/debug/deps/a3s_desktop_lib-104c4ea3ed9a9da3 prompt_stop_reason_tests delivery_contract_tests task_corpus_collect_tests --test-threads=1
```

Initial fresh Work test build: 3.55s. Self-checks: two passed, zero failed, one
ignored collector entry point, 761 filtered out; execution took 0.09s. The checks
cover exact visible text and snapshot bytes, exclusion of reasoning and resume
tokens, A07 hash-only output, no overwrite, failed-turn handling, live or wrong
anchor/session rejection, missing immutable trace, and tampered artifact bytes.
The combined CI filter was then exercised on that same binary: six passed,
zero failed, one ignored collector, 757 filtered out; 0.14s. The additional four
are existing stop-reason and delivery-contract tests, not corpus cases.

The observed reusable test binary is
`target/debug/deps/a3s_desktop_lib-104c4ea3ed9a9da3`. Its ignored entry point can run
after the human UI turn without another build; the portable Cargo invocation and
the required parameters are in `PLAN.md`.

## Prompt-only corpus extension

The closed case list now includes exactly `TC-MAIL`, `TC-COPY`, and
`TC-CONFLICT` in addition to the original 22 office rows. The rejection text
now says 25 office or prompt-only cases. No other collection condition,
artifact behavior, or scoring predicate changed.

Caller review found the ignored parameterized test and its existing
`PLAN.md` invocation. The scorer already contains these three IDs. The
collector remains confined to the explicit persisted terminal, session,
assistant anchor, and immutable artifacts; it does not infer another turn.

The new hermetic fixture removes every tool and proof part, then verifies
all three IDs export only the exact visible text, preserve internal blank
lines, and report no artifact or invented producer status. It also verifies
`TC-ERR-01`, `TC-ERR-02`, and an unknown ID remain rejected. The two ERR rows
retain their manual UI acceptance rules and are not added to this collector
or the scorer. In particular, the collector's deliberate omission of files
on a failed turn cannot prove that the UI showed no completed artifact.

Final focused command after this extension:

```sh
cargo test --locked -p aster-dream --lib task_corpus_collect_tests -- --nocapture --test-threads=1
rustfmt --edition 2024 --check apps/work/src-tauri/src/agent_plane/task_corpus_collect_tests.rs
```

Fresh build: 5.74s. Result: **3 passed, 0 failed, 1 ignored**, 761 filtered
out, 0.20s test time. Scoped rustfmt passed. This extension edited only
`task_corpus_collect_tests.rs` and this evidence file; all concurrent changes
were retained. No actual session, credentials, model, or application was read
or operated by these checks.

## Explicitly not claimed

- The collector was not used to submit a task or operate the UI. The desktop
  operator owns submission, permissions and visible terminal observations.
- Its self-checks do not count toward the corpus E2E pass rate. No corpus answer,
  OCR oracle, source sample or login state was generated or copied by this work.
- The helper does not invent files for an untraced delivery or transform an
  artifact into the expected format. File-format and content grading remain the
  scorer's responsibility, with visual checks performed separately.
- It exports only the explicit current execution anchor and its declared
  immutable snapshots; it does not guess another message or a later workspace
  version from conversation order or filenames.
