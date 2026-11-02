# Feature Specification: Prompt ownership and convergent execution

**Feature Branch**: current checkout; no branch or worktree requested
**Created**: 2026-11-02
**Status**: Complete
**Input**: 深度全面的结合 aster core、aster acp 的机制设计以及 aster tui 和 aster studio 的领域边界，结合业界最佳实践全面优化各处的提示词，并进行验证。理解并采用 Studio 内置提示词优化的方法。
**Gate context**: Product regime. Core owns reusable execution contracts; ACP
adapts host capabilities; TUI and Studio own presentation and domain guidance.

## User Scenarios & Testing

### User Story 1 - Complete bounded work without contradictory instructions (P1)

An explanation stays read-only, an implementation acts and verifies, and a
delegated reviewer reads its target without trying to delegate again.

**Why this priority**: The current ACP implementation-reading ban and inherited
review-dispatch rule conflict with these supported requests.
**Independent Test**: Core prompt assembly and child-policy tests; ACP captured
provider-input tests; real-model explanation, review and implementation cases.

**Acceptance Scenarios**:
1. Given a source-explanation request, inspect and answer without editing.
2. Given an engineering review child, read and report without nested delegation.
3. Given a repair request, preserve all criteria and finish with independent checks.

### User Story 2 - Keep host and domain instructions in their owning layer (P1)

TUI gets terminal Markdown guidance; Studio gets its browser/Office delivery
guidance only with the corresponding admitted capabilities. Core remains reusable.

**Independent Test**: ACP host/terminal/Office matrix, TUI capability tests,
Studio delivery and prompt-generation tests, plus existing completion gates.

**Acceptance Scenarios**:
1. Without Office metadata, do not inject Office tool recipes.
2. With Studio Office capability, preserve file, numerical and visual evidence rules.
3. With terminal presentation, preserve requested HTML artifacts without claiming
   inline rendering. Studio retains its own inline presentation.

### User Story 3 - Improve prompts using the built-in optimization method (P2)

Apply speed, altitude, depth and range to prompt ownership, execution methods
and external acceptance. Preserve original intent, authority, language,
placeholders, output contracts and configured model/budget semantics.

**Independent Test**: Studio optimizer selected-method tests and prompt fidelity
tests; fixed-input real-model cases and recorded usage/call counts.

### Edge Cases

- Unavailable tools and denied permissions stay unavailable; text never grants authority.
- High effort deepens evidence within the task, not the number of speculative tasks.
- A missing prerequisite reports a bounded partial/blocked result, not fake success.
- Compaction, resume and child inheritance retain original goals, scope and evidence.
- Frozen evaluation inputs, models, graders and timeouts are not changed for a pass.

## Requirements

- **FR-001**: Core MUST state scope, evidence, action/report posture and stop
  conditions; refuse invented acceptance goals and unverified completion.
- **FR-002**: ACP MUST permit requested explanation, diagnosis and review; refuse
  unrequested product archaeology and unsupported capabilities.
- **FR-003**: Engineering dispatch MUST target the primary coding session only;
  review children MUST inspect directly and refuse further delegation or mutation.
- **FR-004**: Domain/presentation guidance MUST follow admitted host capabilities;
  generic Core MUST not impose Studio-specific UI or Office workflows.
- **FR-005**: Effort and planning prompts MUST preserve outcome and authorized
  scope; refuse mandatory speculative alternatives or fabricated plan/tool facts.
- **FR-006**: Studio generation/optimization MUST preserve prompt type, language,
  authority and literal placeholders; refuse invented facts, tools and metrics.
- **FR-007**: Verification MUST exercise real assembly and enforcement plus live
  behavior; refuse treating keyword checks or model self-assessment as acceptance.
- **FR-008**: All authored system, workflow and host prompt instructions MUST be
  English. Preserve user/source literals and let user-facing output follow the
  user's requested language; refuse silently changing output-language behavior.
- **FR-009**: Apply Ponytail's full philosophy: eliminate unrequested work,
  reuse before adding mechanisms, solve the complete affected scope and retain
  meaningful checks, error handling, trust validation, security and accessibility.

## House Checklist

- [x] New behavior has a small hermetic test in the owning package (T002–T004)
- [x] Closed sets: no new enum or product configuration
- [x] No production unwrap/expect, dependencies, API or workspace migration planned
- [x] Internal Aster dependencies remain local
- [x] Existing ACL configuration and authority boundaries retained
- [x] Evidence: `docs/evidence/code/2026-11-02-prompt-boundaries/OUTCOME.md` (T005)

## Success Criteria

- **SC-001**: Owning prompt/host/child/completion/compaction regression suites pass.
- **SC-002**: Fixed live cases prove task fidelity, read-only explanations,
  nonrecursive review and validated implementation; record actual usage and time.
- **SC-003**: Rust formatting/Clippy, focused Studio checks and applicable workspace
  checks have retained results; pre-existing failures and native target limits are explicit.
- **SC-004**: An ownership inventory covers active Core, ACP, TUI and Studio
  prompt paths, including retained templates and why they remain unchanged.

## Assumptions

- The built-in optimizer is currently `PromptOptimizeButton.tsx`, not a separate
  installed SKILL.md. Its four selected methods and external acceptance guide this work.
- Use the current dirty worktree as baseline. Preserve unrelated changes and the
  Studio submodule boundary; do not commit without a request.

## Explicitly Not Claimed

- No safety/permission/verification-gate weakening or model-provider migration.
- No prompt-length, latency or reliability improvement without measurements.
- No native Windows/Linux runtime qualification from a macOS run.
