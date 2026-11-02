# Prompt ownership and convergent execution

## Result

Implemented and verified. All four fixed GLM 5.3 scenarios pass in the final
isolated batch: direct response, source explanation, delegated review and repair.
The repair also passes Core's required-check completion gate and an independent
external checker. No measured speedup or universal model-reliability claim is made.

The existing slot, WorkPolicy and capability design is sound. The problems were
cross-layer instructions, conflicting parent/child roles and unnecessary workflow
requirements. The change fixes those sources without adding another prompt framework.

## Changes

- AGENTS.md now defines task-derived acceptance, bounded discovery, project-first
  reuse, the smallest complete change, meaningful checks and a stopping condition.
  Original product, Rust, trust, release and cross-platform requirements remain.
- Core keeps reusable execution contracts. Explanation/review stays read-only;
  continuation records actual open work; planning and effort do not invent scope.
  Catalog guidance no longer imposes Studio forms or rendering on every host.
- ACP permits requested source analysis, distinguishes primary review dispatch
  from direct child work, and projects Office/browser instructions only through
  existing admitted host capabilities. Effort is depth within scope, not a quota
  of alternatives or checks.
- New WorkPolicy bindings use revision 2. Stored policy bytes/digests remain
  authoritative; conflicting explicit rebinds still refuse. Public schemas,
  planning defaults, permissions, budgets, cancellation and completion gates stay.
- Studio's actual built-in optimizer applies only selected speed/altitude/depth/
  range methods. Generator/polish instructions preserve type, authority, language
  and literal placeholders without invented biography or a greeting loop.
  Compaction keeps observed state and does not request private reasoning.
- Authored prompt instructions are English, including inline session capability,
  parameter and room-memory context. User/source literals and exact domain output
  markers remain intact; user-facing replies are not forced to English.
- The English Studio delivery contract is 4257 trimmed characters. The complete
  8000-character caller plus separators and contract is 12259 characters, within
  ACP's unchanged 12288-character limit. Both actual host assemblers verify this.
- GLM traces justified a final review-role refinement: requested dimensions only,
  dependency discovery for concrete uncertainties, and stop after grounded
  findings and coverage. No build environment is required for a source-level finding.

Full active-path inventory and retained-template rationale: [OWNERSHIP.md](OWNERSHIP.md).

## GLM 5.3 behavioral evidence

The user explicitly authorized GLM 5.3. Route `glm-02/glm-5.3` is declared in the
current configuration; native response metadata also identifies `glm-5.3`, not a
Flash variant. Kimi failures remain in their original logs. No configuration
file, account, default model or credential was changed by this task.

Final command, from the repository root with initialized Studio:

```sh
CARGO_TARGET_DIR=target-issue-fix ASTER_CONFIG_FILE=/Users/roylin/code/aster/.aster/config.acl RUST_MIN_STACK=16777216 cargo test --locked -p aster-acp --lib prompt_boundary_real_model -- --ignored --test-threads=1 --nocapture
```

Result: **2 test functions, 4 scenarios, all pass**, in 236.52 s excluding build.
The 300 s source-case and 420 s repair deadlines remain fixed.

| Scenario | Observed duration | Recorded input / output / total tokens |
| --- | --- | --- |
| direct | 11.753 s | 17125 / 296 / 17421 |
| explain | 23.061 s | 33145 / 953 / 34098 |
| review | 156.636 s | 72538 / 5367 / 77905 |
| repair | 18.618 s | 64594 / 200 / 64794 |

- Direct: exact READY, no tools.
- Explanation: actual read, correct four-element/inclusive-range and short-input
  panic analysis, original source bytes unchanged.
- Review: one successful `review` child; observed read/search anchors point to
  `src/limit.rs`. Child progress has read/search/diagnostics, no recursive task.
  The parent verifies against the source, returns file/line findings and leaves
  source unchanged. Existing role permissions also enforce the child ceiling.
- Repair: actual reads of limit.py and check.py, one edit from `[:4]` to `[:3]`,
  actual `python3 check.py` execution with exit 0 and `checks passed`. Core emits
  VerificationSummary Passed, required_check_count 1, pending/failed 0, and
  CompletionTerminal Verified. An external Python process independently passes;
  check.py bytes are unchanged.

Log: [glm53/isolated-live.log](glm53/isolated-live.log).
Parsed observations and log digest: [glm53/results.json](glm53/results.json).
Models, deadlines and source snapshots: [glm53/manifest.json](glm53/manifest.json).
Token counters are recorded observations, not an invoice: ACP flags costIsPartial
and usageIsIncomplete. Timing varies; this is not a controlled performance benchmark.

## Other checks

Counts are separate runs with overlapping coverage and must not be added.
Host: macOS/aarch64-apple-darwin, pinned rustc 1.97.1 (8bab26f4f), package default
features, no feature override. Existing target caches were used; no worktree was created.

| Check | Actual result | Log |
| --- | --- | --- |
| Studio optimizer/generator before edit | 2 failed, 7 passed | `red-studio.log` |
| Studio summary before edit | 2 failed, 11 passed | `red-studio-compaction.log` |
| Core final prompt/child/compaction/convergence/completion/verifier | 132 passed | `glm53/core-scope-final.log` |
| ACP final prompt/policy/host/permission/setup/transport/retrieval | 148 passed, 3 intentionally ignored | `glm53/acp-final-all.log` |
| Final small ACP regressions | 16 passed, 3 intentionally ignored | `glm53/regressions-final.log` |
| TUI initialize and sidecar capability | 5 passed | `tui-capabilities.log` |
| Studio native delivery/parameters/member and room memory | 12 passed | `studio-english-final.log` |
| Studio prompt/editor/token/cancel | 36 passed | `studio-prompts.log` |
| Studio context pipeline | 168 passed | `studio-compaction.log` |
| ACP full caller plus final delivery contract | 1 passed | `host-budget-final.log` |
| Gateway prerequisite caller checks | 99 passed | `gateway-prerequisite.log` |
| Core and ACP all-target Clippy | Exit 0; existing warnings | `glm53/clippy-final.log` |
| ACP after final fixture isolation | Exit 0; existing warnings | `glm53/clippy-isolated-final.log` |
| Studio all-target Clippy | Exit 0; existing warnings | `clippy-studio-final.log` |
| Gateway all-target Clippy | Exit 0; existing warnings | `clippy-gateway.log` |
| Focused Rust formatting, correct package editions | Exit 0 | `glm53/format-final.log`, `task-rust2021-format-final.log`, `studio-rust-format-final.log` |
| Studio focused formatting | Exit 0 | `studio-format-final.log` |
| Studio focused ESLint | Exit 0; 3 existing dynamic i18n-key warnings | `studio-lint.log` |
| Scoped diff whitespace | Exit 0 | `glm53/diff-check.log`, `diff-check-root.log`, `diff-check-studio.log` |
| Workspace dependencies, installed Python 3.13 | 7 passed | `workspace-dependencies-python313.log` |
| Governance regime | 4 passed | `governance.log` |
| Workspace formatting | Unrelated concurrent Studio changes fail | `workspace-format-final.log` |

Required Clippy commands were `cargo clippy --locked -p <package> --all-targets
--no-deps` for aster-core, aster-acp, aster-studio and aster-gateway. Workspace
formatting was `cargo fmt --all -- --check`. Native Studio used its edition 2024;
Core/ACP/Gateway used edition 2021. No full-worktree compliance claim follows.

## Retained failures, fixture repairs and limits

- The attempted pre-change ACP run never acquired another session's build lock.
  `red-acp.log` is not red-test evidence. The existing unlocked cache was used.
- The initial ACP expanded run hit a test-thread stack overflow; the exact case
  and later batches pass with RUST_MIN_STACK=16777216. No production stack or
  evaluation deadline changed.
- An oversized English delivery draft truncated host text. It was shortened while
  preserving the source-fidelity, numerical, real-file, preview and interaction
  requirements. The final complete host payload passes on both sides.
- Revision/digest fixture expectations now bind WorkPolicy revision 2. Validators,
  selection, reopen and conflict checks were preserved; no frozen grader was relaxed.
- The earlier expanded ACP batch had one executable-plugin timeout at the existing
  5000 ms limit. The exact isolated test passed; the final full selected batch also
  passes. The earlier failure remains in `acp-english-final.log`.
- Initial Boyue route setup was undeclared; Kimi then returned monthly quota 429.
  Neither is model-success evidence. Both logs remain; GLM is a separately authorized batch.
- Early GLM review attempts were refused because the headless fixture had no ACP
  client to answer confirmation. Permission rules belong to initialize, not
  session/new. The fixture now uses the existing initialize contract to allow only
  task and explicitly deny write/edit/patch/bash, checking the effective policy
  before review. Production permission enforcement was not changed.
- Two subsequent GLM review attempts reached the fixed 300 s timeout. Health
  observations show provider requests progressing without scheduler queue waits;
  missing child-progress events alone did not prove model inactivity. Later review
  runs pass, but the timeout history precludes claiming reliable speed improvement.
- In-workspace session JSON polluted caller searches. Final fixtures keep session
  stores in separate temporary directories outside source workspaces. Original
  user prompts, SOURCE/CHECK bytes, behavioral assertions and deadlines remain;
  only setup assertions and diagnostics were added. See `glm53/host-prerequisite.json`.
- Temporary synchronous/asynchronous single-flight SDK probes both pass. Their
  source/logs are retained in glm53; the exploratory test edits were removed.
  No speculative scheduler, pool or model-transport implementation was changed.
- Native Studio verification needed three explicit `<str>` Gateway collection
  lookups after an AsRef ambiguity. Gateway tests/Clippy pass. Five subsequent
  stale Studio test API calls were repaired by other concurrent work, not this task.
- The GLM build also needed one borrowed Arc<str> comparison in the already-edited
  plan_replay_tests.rs. Its separate suite reports 3 passed and 1 child-lifecycle
  replay timeout (`glm53/plan-replay.log`); the failing assertion was not changed.
  That unrelated feature needs its own replay investigation; prompt suites pass.
- System Python lacks tomllib; installed Python 3.13 passes the dependency suite.
  Earlier edition-2021 Studio rustfmt mismatches were invocation errors; edition
  2024 passes without formatting unrelated Studio files.
- Workspace-format failures in concurrent Studio document/runtime/macOS files are
  retained, not swept into this change. Clippy also retains existing test mutex
  warnings in Core execution_target.rs:252 and ACP document_run_tests.rs:487/488;
  follow-up is to release those guards before shutdown awaits. Studio's existing
  warnings and dependency block 0.1.6 future incompatibility remain explicit.

## Requirement audit

| Requirement | Authoritative evidence |
| --- | --- |
| FR-001 scope, evidence and stop | Final Core assembly/enforcement regressions and all four inspected live outcomes |
| FR-002 source analysis and capabilities | Real read-only explanation plus ACP host/permission matrix |
| FR-003 primary versus child review | Live successful child with read/search anchors, no child task progress, parent verification; inheritance/reopen/refusal tests |
| FR-004 host/domain ownership | ACP host matrix, real TUI initialize capability tests, Studio complete payload tests and ownership trace |
| FR-005 bounded effort/planning | Core/ACP assembly/transport tests and unchanged configured budget/default semantics |
| FR-006 Studio fidelity | Selected methods, literal multiplicity, stale/cancel/apply, generation and actual summarizer pipeline tests; no universal generated-text guarantee |
| FR-007 actual enforcement and behavior | Real GLM tool events, immutable input/checker assertions, Core Verified effect and independent external checker |
| FR-008 English instruction text | Final source review and English optimizer test; source literals and reply-language behavior preserved |
| FR-009 Ponytail full | Existing mechanisms reused, complete necessary callers/tests retained, no speculative framework or weakened boundary; final source diff |
| SC-001 owning regressions | Final Core 132, ACP 148 and focused Studio/TUI passes, with unrelated failures explicitly separated |
| SC-002 fixed live effects/time/usage | Four passing final GLM scenarios, retained durations/counters, source reads, review child and independently checked repair |
| SC-003 retained checks/limits | Required package Clippy/format/check logs and explicit platform/worktree limits |
| SC-004 active-path inventory | OWNERSHIP.md covers changed and retained Core, ACP, TUI and Studio sources |

## Provenance

[baseline.json](baseline.json) records 44 source snapshots captured before their
respective edits. [after.json](after.json) records 48 final source digests.
[parent-source.patch](parent-source.patch) records the parent repository source
changes selected for publication. The original mixed-checkout comparison and
private Studio source snapshots remain local; they are not added to this parent
repository. Some
pre-existing test files were not captured before their initial edits; their
scoped caller/test changes were also reviewed in the checkout. The private Studio
submodule's published sources remain in that submodule. See
[PUBLISH.md](PUBLISH.md) for the subsequent user-authorized commits and exclusions.

## Explicitly not claimed

- Statistically reliable speedup, universal model reliability or accurate billing totals.
- Native Windows/Linux runtime qualification from this macOS/aarch64 run.
- Whole-worktree formatting/lint compliance or ownership of concurrent edits.
- A prompt replacing permission, sandbox, cancellation or completion enforcement.
- GUI/pixel validation, compatibility-template runtime qualification, release qualification.
