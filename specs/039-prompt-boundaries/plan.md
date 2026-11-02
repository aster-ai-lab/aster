# Implementation Plan: Prompt ownership and convergent execution

**Branch**: current checkout | **Spec**: [spec.md](spec.md)
**Gate context**: Root product workspace and private Studio submodule.

## Summary

Keep existing slot assembly, WorkPolicy inheritance, capability metadata, plans,
budgets and completion gates. Fix prompt scope at its source; route domain text
through existing host metadata. Apply Studio's speed/altitude/depth/range method
with external checks, rather than adding personas or compulsory process.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Preserve unavailable tools, refusals and completion gates |
| II. Evidence retained | PASS | Frozen source baseline and final OUTCOME/logs |
| III. Hermetic tests | PASS | T002–T004 exercise existing assembly and host code |
| IV. Closed sets | PASS | No new admitted-value type |
| V. Workspaces / APIs | PASS | Existing crates, explicit Studio submodule ownership |
| VI. Schema changes | N-A | No database schema changes |
| VII. Shrink-only ratchets | PASS | No allowlist or frozen grader changes |
| VIII. Transports | PASS | Existing ACP metadata, no provider migration |
| IX. ACL | PASS | No new product configuration |
| XIII. One repository | PASS | Current checkout; Studio sources stay in submodule |
| XIV. Names not guessed | PASS | Preserve exact source/user values |
| XV. Meaning not matched | PASS | Semantic task guidance, no new routing regex |
| XVI. Pickers | N-A | No widget changes |
| XVII. Dialogs | N-A | No motion changes |

## Project Structure

Planning: this directory's spec, plan and tasks. Acceptance is recorded only in
`docs/evidence/code/2026-11-02-prompt-boundaries/`.

| Layer | Existing source and pattern | Planned change |
| --- | --- | --- |
| Core | `crates/code/core/prompts/`, `src/prompts.rs` | Scope/stop contracts, specialty/continuation/planning text; preserve slot APIs |
| ACP | `crates/code/acp/prompts/engineering.md`, `src/host_context.rs`, `src/agent.rs`, `src/agent/models.rs` | Nonrecursive engineering policy, scoped domain instructions, proportional effort |
| TUI | `crates/terminal/crates/codegen/aster-pager/src/acp/mod.rs` → ACP terminal contract | Validate existing terminal capability and presentation; do not hand-edit generated artifacts |
| Studio | `apps/studio/src-tauri/src/agent_plane/delivery_contract.md` | Preserve domain acceptance; conditional workflow instead of global coding process |
| Studio optimizer | `src/renderer/components/PromptOptimizeButton.tsx`, `src/shared/ai/prompts.ts`, resource prompt constants | Preserve types/literals and apply only selected optimization methods |

## Research

- Studio optimizer: speed removes wasted waiting; altitude targets real outcomes;
  depth uses authoritative inputs and independent checks; range adds methods only
  when they change the result. It forbids fabricated facts/tools/permissions.
- OpenAI prompt-engineering guidance: roles, explicit boundaries, current tool
  schemas and model-specific behavioral evaluations.
  https://developers.openai.com/api/docs/guides/prompt-engineering
- Anthropic context-engineering guidance: minimal sufficient, distinct context;
  avoid brittle laundry lists and keep runtime controls outside prompts.
  https://www.anthropic.com/research/effective-context-engineering-for-ai-agents
- Existing runtime is authoritative: `agent/prompt_runtime.rs`,
  `agent/turn_context.rs`, `fact_control.rs`, `child_run.rs`, completion gates.
- Retain planning defaults initially. Optimize their output/scope without silently
  disabling public Auto semantics. Measure overhead rather than asserting a speedup.

### Ponytail philosophy applied

| Principle | Concrete application | Acceptance evidence |
| --- | --- | --- |
| First ask whether work is needed | No compulsory planning, personas, alternative proposals or repeated discovery for simple tasks | Selected-method tests and fixed simple-task live case |
| Reuse before adding | Keep slots, WorkPolicy, host flags, runtime gates and existing test runners | No dependency/configuration/API additions; final task diff |
| Smallest complete change | Preserve affected callers, fixtures, format, language and literal placeholders | Host matrix, optimizer/resource tests and implementation effects |
| Fix the shared cause | Remove host-specific rules from Core, scope review dispatch at its source | Actual prompt assembly and inherited child-policy tests |
| Lazy solution, rigorous completion | Observe and verify effects; no model self-certification or test weakening | Completion/permission regressions and live effect assertions |
| Never cut essential guarantees | Keep validation, errors, security, accessibility, cancellation and budgets | Existing enforcement suites remain unchanged |
| Stop when done | No new scope or repeated successful checks without evidence | Fixed live traces and bounded review/continuation cases |

Spec/plan/tasks consistency review: FR-001–FR-009 map to T002–T005, including
English instruction text and Ponytail constraints. No constitution blocker or
unowned behavior change was identified. Ready for implementation; final evidence
and cross-platform limitations remain T005/T006 obligations.

## Verification Plan

- Capture worktree prompt baseline before edits (34 source snapshots retained).
- Add regression cases before changing production prompts; retain expected red output.
- Run Core prompt, child-policy, compaction, convergence and verification tests.
- Run ACP prompt/host/engineering/effort tests and TUI terminal capability checks.
- Run Studio optimizer/resource/host delivery focused tests and relevant linters.
- Run pinned-toolchain format/Clippy and existing dependency/governance checks.
- Run fixed live-model cases through actual Core/ACP host paths; retain inputs,
  request capture where supported, actual tools/effects, usage and failures.
- Native host is macOS; Windows/Linux runtime checks remain separately reported.
