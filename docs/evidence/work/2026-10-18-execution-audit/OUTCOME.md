# AsterDream task execution audit

## Status

Active. The objective is the complete task execution path, not just the earlier
artifact repair. Requirements and remaining work are retained in
[`specs/001-task-execution`](../../../../specs/001-task-execution/spec.md).
No completion-rate improvement is claimed without a new live corpus run.

This workspace has concurrent structural migrations and unrelated changes.
Only the named behavior and focused commands below belong to this audit.

## Confirmed behavior changes

- ACP permission, question and form waits now observe Stop. Cancellation skips
  new questions and still settles the original worker after event EOF.
- Core detached model calls can be cancelled during stream setup and fallback.
  Typed budget/permanent refusals do not invoke fallback. A stream that opens
  and then ends without Done is a failure, not another completion attempt.
  Dropping the call future cancels its provider child without cancelling the
  caller's host token.
- The headless protocol distinguishes accepting a cancellation request from
  the original worker actually retiring. It pins the immutable original Run
  binding, including after its live slots disappear, and waits within a finite
  deadline. A timeout or poisoned identity registry returns typed pending
  failure, retains the owner, and produces no terminal success receipt.
  The public error variants and const `code()` contract are unchanged; the
  additive `runtime_code()` distinguishes the typed pending source from prose.
- Native Office now supports DOCX paragraphs/tables, explicit XLSX formulas and
  formats, and PPTX embedded chart images. The live Write scope and captured
  workspace identity are rechecked. A real Harness test exposed that creating
  a valid file previously still lacked the completion gate's bound verification.
  A default-disabled Rust Tool hook now supplies native validation evidence;
  Core independently matches the actual typed receipt, saved-byte digest and
  file identity. Only all-path coverage can certify the current aggregate
  mutation digest. Mixed unverified edits, incomplete observations, stale files
  and forged external metadata cannot acquire that proof. The Gate is unchanged.
- Core forced-stop reasons now reach ACP: truncated output reports max_tokens;
  exhausted tool/evidence requests report max_turn_requests. Work succeeds only
  for explicit end_turn; missing/unknown reasons, refusal and resource stops
  preserve partial text with a failed terminal rather than success.

## Current validation

All Rust commands run at the repository root with the pinned toolchain.

| Command / evidence | Observation |
| --- | --- |
| Fresh ACP binary `a3s_code_acp-c44cc472c5433f35 turn_terminal_tests turn_cancel::tests --nocapture` | 5 + 3 passed, 0.80 s; six interactive permission/question/form scenarios plus all three typed forced stops |
| Core `fact_control::model_error_tests` and `fact_control::model_governance_tests` | 5 + 6 passed; setup/fallback/EOF/cancellation/drop and neighboring delivery behavior |
| Initial Core `protocol_cancel_tests` | 2 passed, 1.03 s; this preceded the additional retired-slot and poison regressions and is not final coverage |
| `cargo test --locked --offline -p a3s-code-core --lib -- protocol_cancel_tests agent_protocol_host::`, final fresh binary rerun with the same filters | 7 passed, 1.02 s; accepted cancellation, terminal/slot retirement before worker exit, pending deadline, poisoned registry, typed-source classification, foreign-session identity refusal and API compatibility |
| Fresh Core test binary: `context::assembler agent_api::conversation_runtime::memory_retention_tests task_scheduler::tests --nocapture` | 63 passed, 1.77 s; bounded/trusted context selection, both memory adapters, original committer/retention lifecycle, admission/cancellation/fairness/quota bounds |
| Delegation/compaction/budget focused groups | 68 passed; exact commands in run-control MECHANISMS.md, including the 28-branch fanout and the final two source-digest/soak tests |
| Explicit fanout cancellation soak | Initial final-digest failure isolated to 320 legitimate child harness-state files; test helper now reuses the existing harness-path boundary. Real source and ordinary hidden-source changes still change the digest. Final 20×8 cancellation soak and helper regression: 2 passed, 8.86 s; included in 68, not counted twice |
| `cargo test -p a3s-code-core --lib -- native_artifact_verification_tests verification_metadata_tests --nocapture` | 13 passed, 0.02 s; four new adversarial native cases and nine neighboring report-contract cases |
| `cargo test --locked --offline -p a3s-code-acp --lib -- office_file output_check artifact_image source_review_has_no_tools_even_after_a_yolo_mode_change` | 25 passed, 0.70 s; real live-Harness Verified result, write/deny/read-only/host-direct refusals, format save/reopen, numeric and SVG limits |
| `cargo test --locked --offline -p a3s-office --lib image_tests` | 6 passed; additive native image positioning, preserved defaults, round-trip and format/bounds refusals |
| Work ordinary `prompt_stop_reason_tests delivery_contract_tests` | 4 passed on the same fresh Work test binary, 0.00 s |
| Work→actual ACP/Core→loopback provider, exact `agent_plane::turns::local_acp_tests::delivery::` filter with `--ignored` | Final 3 passed, 1.85 s; normal native delivery, invalid extension/provider failure, and response truncation |
| `a3s-test capabilities --json` | Infrastructure refusal: installed browser 0.3.2; adapter requires >=0.4.0,<0.5.0. No Web test executed |
| `a3s-test gui-certification --json` | macOS installed/embedded profiles are contract-tested; this is a capability report, not this application's live GUI certification |
| CUA inventory and AsterDream AX observation | Existing app is running, logged in, Qwen 3.8 Flash selected. Earlier absence of provider environment variables does not prove the desktop has no usable model. No new task has been submitted in this observation |
| `cargo fmt --all -- --check` | Passed before the later concurrent OCI sandbox module was introduced; this does not certify that new migration's formatting |
| `cargo build --locked --offline -p aster -p a3s-code-acp -p a3s-office-cli` | Passed before the subsequent concurrent OCI sandbox additions; terminal and both companions built together |
| `cargo check --offline -p a3s-box-core` | Passed after the inherited transport alias was made local |
| `python3.13 tests/workspace_dependencies.py` | Final run: 6 passed, 1.481 s. Earlier failures identified the missing publish flags and remote inherited alias and were fixed without changing the guard |
| Latest `just build-work` attempts | First blocked by newly declared `sandbox::oci` missing its file. After that file appeared, the next attempt failed on its missing `bundle`/`output` submodules and `signal.number()` on i32. These are concurrent migration files outside this audit, not reverted or bypassed. Final shipped-host acceptance remains open |

The dependency guard also exposed 12 newly incorporated manifests without
`publish = false` and an inherited `a3s-transport` alias resolving `a3s-common`
from the registry. The root instructions explicitly require all members to be
unpublishable and internal A3S dependencies to be local. This audit added only
the missing publish flags and the alias's existing local path; the dependency
guard and local Box check now pass, including the guard's lockfile check. No
package rename, public API replacement or migration rollback was performed.

The real pipeline's successful case observed 1130 ms, seven provider calls
(six streaming task calls plus one actual non-streaming memory extraction),
five tool calls and four captured artifacts totaling 14551 bytes: DOCX 2722,
XLSX 3263, PPTX 7560 and PNG 1006. Each snapshot matched the workspace bytes.
The invalid-extension/provider-400 case observed 409 ms, two provider calls,
one tool call and zero artifacts. Truncation observed 274 ms, one provider
call, zero tools and zero artifacts. These are single local loopback
observations, not live-model performance claims. The ACP executable used for
this proof has SHA256
`ee5893698ecfc7431a2f897fd255fc27b427ccfe0e719d7da2a4e592317ceb97`;
it does not contain the later unverified concurrent OCI migration.

Cancellation test details are retained in
[`2026-10-18-run-control-audit`](../../execution/2026-10-18-run-control-audit/OUTCOME.md).
Earlier delivery, binary builds and their exact limitations remain in
[`2026-10-18-task-delivery-optimization`](../2026-10-18-task-delivery-optimization/OUTCOME.md).
Actual pipeline fixtures and results are retained in
[`2026-10-18-real-acp-delivery`](../2026-10-18-real-acp-delivery/OUTCOME.md).

## Requirement acceptance matrix

“Partial” means direct evidence covers some paths; it does not certify the
complete requirement. A controlled provider proves runtime wiring, not an
autonomous model's tool choices or answer quality.

| Requirement | Direct evidence / remaining acceptance | Status |
| --- | --- | --- |
| FR-001 admitted capabilities | ACP registration/permission tests, actual pipeline and live Turn authority tests pass; conditional tool/plugin combinations remain | Partial |
| FR-002 usable task families | Real DOCX/XLSX/PPTX/PNG save/reopen and actual delivery pass; generic local Office input and independent XLSX/PPTX reviewer remain | Partial |
| FR-003 grounding, wording, numeric and length | Delivery contract, bounded deterministic numeric/Unicode checks; live original task outcomes not replayed | Partial |
| FR-004 planning/delegation ceilings | 68 focused mechanisms, including shared eight-slot/28-call fanout and 20×8 cancellation soak; other background/recovery combinations remain | Partial |
| FR-005 finite budgets and classified retries | Eleven Core detached/governance tests; full delegated/resource coverage pending | Partial |
| FR-006 cancellation/control/recovery | Eight ACP tests and seven protocol cancellation/error tests pass; broader steering/recovery combinations and recovered incremental native proof remain | Partial |
| FR-007 context/compaction/memory | 35 compaction tests plus current context/memory/scheduler group of 63 and actual background memory request; live long-task quality remains | Partial |
| FR-008 completion with actual results | Actual Work native delivery and failure/truncation scenarios pass; original corpus and visual artifact opening remain | Partial |
| FR-009 correlated bounded diagnostics | Existing context reports/run facts, reason diagnostics and measured actual pipeline counts/bytes/time; complete observability audit remains | Partial |
| FR-010 evaluation layers | Hermetic and real pipeline tests pass; live corpus and independent OCR oracle gates remain | Partial |
| FR-011 shipped companion/checks | Paired build passed before later OCI changes; dependency guard passes. Current `just build-work` is blocked by the unrelated in-progress OCI migration | Partial |

## Explicitly not claimed

- No new 22-task live completion rate, all-mechanism certification, desktop
  release qualification, cross-platform rendering proof or whole-CI pass.
- The logged-in running desktop has not yet been proved to be the rebuilt host
  with these final changes. Inventory/AX output alone does not establish that.
- Missing OCR oracles are not synthesized. Task source files and scoring rules
  are not changed to improve the score.
- Native macOS Bash still requires a qualified descendant-lifetime backend.
  Full access is not such proof; new in-process capabilities do not certify
  arbitrary command execution or the unavailable guest image/shim.
- Aggregate token accounting is an explicitly soft ceiling: already in-flight
  parallel calls may finish above it; the next check refuses further work.
  Tests of budget admission are not proof of a hard aggregate token ceiling.
- General local Office input parsing is still conditional on installed document
  tools or the document host. The new artifact creator does not by itself
  replace an unconfigured remote parser for arbitrary uploaded Office files.
- Work's independent artifact reviewer currently extracts images and DOCX;
  XLSX/PPTX review with `reviewMode=true` is not proved. The controlled delivery
  cases use `reviewMode=false` and trace/capture actual saved bytes.
- Native proof maps intentionally do not deserialize authority from JSON.
  Recovery with additional new artifacts may need fresh verification; it must
  not infer cumulative proof from a restored cache. This remains acceptance work.
- The A3S Test browser version refusal is not a product failure and is not
  bypassed by changing the tool's compatibility gate.
- Scoped tests cannot certify arbitrary model grounding, every Office layout,
  general artifact editing, optional external services or all scheduler modes.
