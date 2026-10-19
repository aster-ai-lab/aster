# AsterDream task delivery optimization

## Result

The reported failures exposed missing execution and delivery capabilities in
addition to model instruction gaps. Ordinary ACP sessions now expose two
in-process tools, without depending on macOS Bash:

- `office_file`: creates real XLSX, PPTX and PNG files, saves and checks bytes,
  reopens Office packages, preserves literal spreadsheet strings, and publishes
  only new files inside the writable local workspace. Existing files, denied
  paths, missing session authority and symbolic links are refused.
- `output_check`: measures Unicode text length and computes percentage changes.
  Missing inputs and zero baselines stay undefined; prose uses ASCII signs and
  omits insignificant decimal zeros. Numeric results remain numeric.

File tools are classified as mutations in interactive, specialty, verifier and
fact permission paths. Source-review sessions expose neither tool. ACP reports
`office_file` as an Edit with its original input path, allowing Work's existing
artifact capture to retain the saved files with XLSX/PPTX/PNG MIME types.

Presenton's shipped language list uses single quotes. The host previously read
only double-quoted strings and obtained an empty list. The corrected mechanical
parser and tool schema share all 90 actual language values. A preview request
acknowledgement no longer claims that a PPTX file has been delivered.

Core resets final-text delivery state for every response, including
non-streaming fallback, ignores empty deltas for suppression, and stops reading
when the provider emits Done. ACP refuses clean worker EOF without a terminal
event, and cancellation still works after the event channel closes while the
worker retains its run permit. These changes do not forge `turn.complete` or
waive completion gates for failed tools.

Work's standing delivery requirements cover requested fields, original names,
dates and units, missing facts, source/proposal separation, numeric checks,
length checks, separate OCR and summary answers, real file formats, readback and
an explicit final answer. The existing 8000-character caller allowance is
preserved; ACP's bounded host envelope now permits 12000 characters to include
the 2529-character delivery contract alongside it. No task-number classifier or
meaning matcher was added.

## Validation

All commands ran from the repository root with the pinned Rust toolchain,
except the renderer command whose working directory is `apps/work`.

| Command or exact test-binary filter | Result |
| --- | --- |
| `cargo test --locked --offline -p a3s-code-acp --lib office_file` | 8 passed; real XLSX/PPTX/PNG, saved readback, geometry, no-clobber, cancellation, write/session authority, symlinks and replaced workspace root |
| Fresh ACP binary `target/debug/deps/a3s_code_acp-7fdaa69d7ef354e3 output_check artifact_image turn_terminal_tests host_context::tests tool_cards_use_pager_kind_and_raw_input source_review_has_no_tools_even_after_a_yolo_mode_change --nocapture` | 21 passed |
| `cargo test --locked --offline -p aster-dream --lib delivery_contract_tests` | 2 passed, including full caller allowance and retained source-review restriction |
| `cargo test --locked --offline -p aster-dream --lib presenton_mcp` | 4 passed |
| `cargo test --locked --offline -p aster-dream --lib standing_host_context_stays_out_of_the_user_text` | 1 passed |
| `cargo test --locked --offline -p aster-dream --lib tobot_persona_changes_the_role_and_stays_off_task_sessions` | 1 passed |
| `cargo test --locked --offline -p aster-dream --lib office_file_inputs_are_captured_as_typed_binary_artifacts` | 1 passed |
| Fresh Work binary, the five preceding filter groups together | 9 passed; duplicate rerun, not 9 additional tests |
| `/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx` | 3 passed |
| Current PNG and metrics modules independently compiled with `rustc --test` | 4 PNG and 3 metrics passed; duplicate coverage, not extra tests |
| `cargo build --locked --offline -p aster -p a3s-code-acp -p a3s-office-cli` (final retry) | Passed; terminal and both companions built together |
| `just build-work` | Passed; ACP and Office built first, then the Work host; updated companions copied into host resources |

Core terminal-delivery and authorization tests are recorded in
`../2026-10-18-terminal-delivery/OUTCOME.md` and
`../2026-10-18-output-authorization/OUTCOME.md`. Native execution diagnosis,
PNG limits, and artifact implementation details are recorded in
`../2026-10-18-dream-native-execution-diagnostics/OUTCOME.md`,
`../2026-10-18-native-png/OUTCOME.md`, and
`../2026-10-18-native-artifact-delivery/OUTCOME.md`.

Scoped `rustfmt --check` and `git diff --check` passed. The workspace was being
edited concurrently: early dependency-resolution failures and a later missing
`A3S_INFERENCE_PROVIDER` import interrupted builds before subsequent successful
owning-package builds. These intermediate failures are retained in the slice
records rather than counted as product test passes.

The final attempted native-boundary run,
`cargo test --locked --offline -p a3s-code-core --lib sandbox::`, stopped while
compiling an unrelated concurrently changed test at
`agent_api/run_admission/owner_lifetime_tests/protocol_cancel_tests.rs:69`:
`AgentProtocolHostError` has no method `runtime_code`. No native-boundary test
pass is inferred from this failed command.

`cargo fmt --all -- --check` remains non-green on unrelated Cloud/Box formatting
and the unresolved `crates/use/tests/../crates/extension/src/tuf_test_support.rs`
module path. `python3.13 tests/workspace_dependencies.py` runs 6 tests, 5 pass and
1 fails because `a3s-cloud-contracts` has an unspecified publish field. No
allowlist or unrelated migration was changed to make these checks pass.

## Concurrent changes preserved

The original bundled `sheet-compute` script was initially fixed and two small
Python regressions passed. Another workspace change then deleted that entire
skill. This task preserved the deletion and removed its own now-dangling Python
test. The retained implementation uses the native measurement tool instead;
the discarded script change is not claimed as delivered.

## Explicitly not claimed

- No live provider or complete replay of the 22 tasks was run. No API credential
  was present in the task environment. No completion-rate increase is claimed.
- A06's missing oracle is not generated or treated as a pass. Neither scoring
  rules nor task answer files were modified.
- Instruction coverage does not prove that every future model answer preserves
  every required phrase, avoids every unsupported claim, or obeys every limit.
- macOS Bash remains refused without a qualified descendant-lifetime backend.
  Full access does not weaken that gate. Guest diagnostics now retain the actual
  missing backend reason and explain the available in-process alternative.
- PPTX creation uses the existing native text layout and bounded content; it is
  not a reproduction of a Presenton preview or a general presentation designer.
- PNG rendering is bounded and uses the existing system font fallback. It is
  not an independently isolated process or a cross-platform font qualification.
- No full CI, release/package installation, or whole-workspace build pass is
  claimed from the focused tests.
