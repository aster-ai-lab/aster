# Permission mode propagation and duplicate confirmation

## Scope

The task covers Aster's four native authorization modes: `default`, `plan`,
`auto`, and Studio `bypassPermissions` / ACP `yolo`. It does not change the
permission modes of third-party coding tools.

The initial explanation that ACP lacked a YOLO auto-approval branch was
incomplete. Core already waives tool-owned confirmation when the admitted
Run explicitly permits unsandboxed execution and allows that exact call.
An ACP confirmation request must continue to reach the user; Studio must not
turn an outstanding Core Ask into Allow based on a later UI selection.

## Changes

- Edited-message resend now carries the selected permission mode through the
  same IPC contract as a new turn. Before the fix, the renderer dropped it
  and the edit-resend schema rejected it. All four renderer regressions failed
  before the fix, and the schema probes exposed both missing and unknown-mode
  handling.
- The stream and edit-resend boundaries accept the same closed set. Their
  TypeScript request types now agree. Unknown, empty, null, and non-string
  modes refuse instead of silently keeping the old mode.
- Live interjections retain an explicitly selected mode for the next Run;
  they do not alter the open Run. The picker now states this timing in both
  supported UI languages instead of promising the next action.
- The native host propagates mode persistence errors before starting a turn.
  The store commits its in-memory mode only after the existing file write
  succeeds. A failed save keeps the preceding mode.
- The English runtime contract tells the model to use the host approval flow
  for authorized tool actions instead of requesting duplicate chat/form
  confirmation. Missing task decisions and explicitly requested plan reviews
  still require an answer.

No blanket Studio auto-approval, new dependencies, permission bypass, or
change to the completion gate was added.

## Mode contract

| Mode | Workspace reads | Workspace writes | Default sandboxed Bash | Host execution / network grant |
| --- | --- | --- | --- | --- |
| Normal (`default`) | Allowed | Confirm | Confirm | Confirm |
| Plan (`plan`) | Allowed | Denied | Denied | Denied |
| Auto (`auto`) | Allowed | Allowed | Allowed | Confirm |
| Full access (`yolo`) | Allowed | Allowed | Host runner | Allowed by the admitted Run |

Explicit deny rules and the catastrophic-command floor remain effective.
Approval of a command does not answer an `ask_user` question. The independent
"plan first" option still waits for the requested plan review. A mode change
applies to the next admitted Run, leaving an existing Run's frozen authority
intact.

The design references are the official [Claude Code permission documentation](https://code.claude.com/docs/en/permissions)
(host-enforced rules and separate approval modes) and the official
[Gemini CLI policy engine](https://geminicli.com/docs/reference/policy-engine/)
(mode-specific policies and context-aware approvals). Aster keeps its existing
four-mode contract and conservative boundaries.

## Verification

Native host: macOS aarch64, pinned Rust 1.97.1. Cargo uses `--locked` and
`RUST_MIN_STACK=16777216`. The process-level tests launch the actual ACP
executable with a loopback HTTP provider and isolated workspace/state/home.
They do not use unattended approval.

- [Renderer regression before the fix](renderer-before.log): all four edited
  resend mode cases failed because permissionMode was missing.
- [IPC regression before the fix](schema-before.log): missing edit-resend field
  and unknown stream mode were exposed.
- [Renderer after the fix](renderer-final.log): 28 passed across runtime and
  permission posture tests.
- [IPC after the fix](schema-final.log): 29 passed, including both request paths
  and all four modes.
- [TypeScript checks](typecheck-final.log): the shared checkout still has 258
  errors. The initial run also found two errors introduced by this task; those
  were fixed and are absent from the rerun. Four existing topic-status mock
  errors remain in the touched renderer test file.
- [ESLint](eslint-final.log): no errors, two existing warnings in the runtime
  hook. [Oxlint](oxlint-final.log) and [TypeScript formatting](ts-format.log)
  pass for the changed TypeScript paths.

The initial [real ACP process suite](acp-all-e2e-final.log) passed 29 tests.
The [final shared-checkout replay](acp-all-e2e-recheck.log) passes 32 tests;
three concurrent convergence cases were added to that checkout. Only this
task's two new permission test functions are selected for publication.
The two new functions cover 24 operation/mode checks (including transitions
back to Normal and Full access) and three rejected commands. Host commands
require actual successful receipts with exit 0; denied calls never create
their files or produce a successful command receipt.

The intermediate [GLM5.3 run](glm53.log) passed all four cases. After
compacting the new approval rule to meet the unchanged prompt budget, the
[final frozen evaluation](glm53-final.log) accepts three of four cases:
direct reply, source explanation, and repair with the actual external checker.
Review completes through the review worker, whose task metadata records
`read` and `search` source anchors for `src/limit.rs`, but the frozen assertion
at `prompt_boundary_tests.rs:164` counts only the parent's tools (`task`).
We classify this as an evaluator coverage defect; the recorded result remains
failed. [The final summary](glm53-final-summary.json) preserves that distinction.
The follow-up is to version an evaluator that inspects committed child
receipts; this task leaves the grader unchanged.

The model remains `glm-02/glm-5.3`, with the original 300/420 second limits
and original task/checker bytes. [The input digest](glm53-inputs.json) matches
the earlier frozen campaign. No evaluator source or model binding changed.

The [A3S TUI replay](a3s-run.json) also passes both functions and all 27
operation checks, with process exit 0 and no output truncation. Its owned
[terminal recording](terminal/permission-results.vt) is copied byte for byte;
[a3s-artifacts.json](a3s-artifacts.json) retains its digest.

Final [A3S replay](a3s-run-final.json) also passes with exit 0 and no
truncation; its [recording](terminal/permission-results-final.vt) comes only
from that owned run.

| Final check | Result | Log |
| --- | --- | --- |
| Native host mode, save-failure, invalid-input and live-mode tests | 4 passed | `studio-permissions-final.log` |
| Existing native mode resolution | 1 passed | `studio-existing-mode.log` |
| Core permission postures | 2 passed | `core-postures.log` |
| Core prompt contracts and unchanged 12 KB budget | 48 passed | `core-prompts-final.log` |
| ACP permission, question and cancellation units | 29 passed | `acp-permission-units.log` |
| Core + ACP all-target Clippy | Exit 0, existing warnings | `clippy-core-acp-final.log` |
| Studio all-target Clippy | Exit 0, existing warnings | `clippy-studio-final.log` |
| CLI + ACP paired build | Exit 0 | `build-pair-final.log` |
| Workspace dependencies / governance | 7 / 4 passed | `workspace-dependencies.log`, `governance.log` |
| Rust formatting / UI translations | Exit 0 | `format-final.log`, `i18n-check.log` |

Exact Cargo commands, exits and durations are in [checks.json](checks.json).
The first ACP matrix probe had a fixture defect: it consumed a pending tool
on a model request without tool definitions. The fixed fixture reserves the
operation for a tool-capable request and gives each tool call a unique ID.
The preserved failure logs are not claimed as a Core permission regression.
The first native live-mode probe timed out during fixture startup before any
ACP prompt. It now uses the existing production `prewarm` API before the same
10 s admission assertion; the four live-mode cases then pass. A direct Node
probe found no interpreter defect; no launcher code changed.
The first prompt-contract run exposed a 12,195-byte prompt. The final rule
was merged into the existing authority bullet instead of increasing the
12,000-byte limit. The failed run is retained in `core-prompts.log`.
An intermediate Studio Clippy run found an unrelated PathBuf/string assertion
in `code_cli_config_tests.rs`; a concurrent edit fixed it before the final
run. That file is not part of this change.

The desktop cold build was stopped after its compiler waited while macOS
loaded a procedure-macro library. The warm-cache retry then waited on an
external build lock; only that waiting test was stopped. The final retry
completed using `target-goal-issues`. No other session was stopped. A sample of the
new ACP test executable showed it at `_dyld_start` before test entry; its
29 tests later ran and passed with their original deadlines.

## Publication and limits

The workspace contains extensive concurrent changes. Only this task's delta
is selected against the recorded before snapshots. The recorded tests use the
shared checkout, not a clean checkout of a partially staged tree.

Studio commit `43bb70e39670955731e546027844f3caca0808f9` is signed and
DCO-signed off, and is published on its remote `main`. GitHub reports
`verified: false` with `unknown_key`: its account has not registered the
signing public key. This task does not change account signing-key settings.

## Explicitly not claimed

- A clean full TypeScript/workspace gate; the reported type errors remain.
- A 4/4 pass under the final frozen GLM grader; review remains marked failed
  for the parent-only source-inspection assertion described above.
- Windows/Linux native runtime qualification or GUI interaction with a user's
  running Studio window. The process matrix exercises ACP, while the renderer
  and native host have separate focused tests.
- Real network grant execution or a real-provider outage. The live GLM cases
  are focused regressions, not a general model reliability claim.
- Automatic answers to user questions, approval of an unreviewed plan, or a
  waiver of explicit denies, hooks, sandbox constraints, or completion evidence.
