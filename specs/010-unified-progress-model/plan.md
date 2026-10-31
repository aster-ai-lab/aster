# Implementation Plan: Unified progress model (v2, reviewed)

**Spec**: specs/010-unified-progress-model/spec.md · **Baseline**: user-verified
deadlock (2026-10-22 home-server freeze) + slice-1 machinery in tree.

## Self-review findings on v1 (each addressed below)

| # | v1 defect | Fix in |
| --- | --- | --- |
| R1 | "information Δ" can't distinguish productive exploration from endless thrash (re-reading new files forever = positive Δ, still a spin) | P1: bounded diminishing-returns window + multi-dimension stall (zero across ALL four Δs, not just reads) |
| R2 | TerminateStalled could be classified RetryNonTerminal by L2 — resurrecting the restart-spin the user originally reported | P2: stalled is a NON-RETRYABLE terminal class in L2 classification |
| R3 | Policy structs as runtime config would open a workspace-layer widening surface (untrusted layer loosening stall limits = deadloop back) | P3: policy values are compile-time constants with provenance; host layer may only TIGHTEN |
| R4 | Satisfiability matrix has no mechanism forcing new rules to register their dimension | P4: dimension registry enforced by the closed-set lint (a new gate without a matrix row fails CI) |
| R5 | v1 omitted observability, field rollback, the concurrent ~16-failure repair dependency, slice-1 closure, and bench fixture provenance | P0 + P1 below |

## Phase 0 — Close the foundation (blocks everything)

- Slice-1 compile+battery green (wiring written; verification pending on the
  shared build lock), commit guard + slice-1 together.
- Dependency: the concurrent lane's ~16 pre-existing core-lib failures get a
  repair lane BEFORE P4 (otherwise the matrix reads noise). Coordinate: this
  is upstream of P4 only; P1–P3 proceed regardless.

## Phase 1 — ProgressLedger (additive, zero behavior change)

**Touchpoints**: `harness_loop.rs` (extend MutationLedger at the existing
`settle_prior_turns` hook), new `progress.rs`.

**Delta semantics (exact, cheap, local-only)**:
- `information_delta`: count of tool calls this turn whose
  (tool, normalized-target) pair is FIRST-SEEN within a sliding window of
  the last `W=8` turns; repeat reads inside the window count 0. Bound: O(1)
  per call against a bounded LRU (cap 512 pairs).
- `mutation_delta`: verified effects this turn (already in the ledger).
- `uncertainty_delta`: net decrease of unresolved+recorded obligations.
- `plan_delta`: assistant text emitted OR update_plan/summary tool called
  with content differing beyond a 0.9 jaccard on shingles vs the previous
  (bounded 4 KB tail; no model calls).
- `TurnProgressDelta` lands in the fact log (`progress.delta` fact) → TUI
  trace "why is this run continuing" (R5 observability).

**Acceptance**: hermetic tests per dimension incl. the R1 case (endless
novel reads → information_delta > 0 but the STALL predicate in P2 requires
all four zero, so exploration alone can't be terminated; the bounded window
keeps the signal honest). Existing batteries green. No decision consumer.

## Phase 2 — L3 admission consumes progress (deadlock terminator)

**Discovery while implementing (2026-10-22)**: the stall exit ALREADY
exists — `ContinuationUsage.slices_since_progress` over settled slices,
`max_no_progress_slices = 3`, one replan earns one more slice
(`goal_control/continuation.rs`, `facts/run/continuation.rs`), and the
extension point is the `ProgressBasis` closed set whose single member is
`ObservedWorkspaceMutationV1` (mutation-only progress — the freeze
scenario's counterweight: blocked mutations starve the basis, and slices
that cannot settle do not count). P2 is therefore a BASIS UPGRADE, not a
new mechanism: add `ProgressBasis::TurnProgressDeltaV1` (a slice makes
progress when ANY of the four P1 deltas is nonzero), feed it from the
settle watermark's returned delta, and keep the existing termination,
replan grace, and admission checks untouched.

**Touchpoints**: `goal_control/facts.rs` admit path,
`GoalTickDecisionV1::TerminateStalled` (closed set + parse/as_str),
`unattended/result.rs` terminal classification (R2: NON-RETRYABLE).

**Decision**: `AdmissionPolicy::Stall { max_zero_delta_slices = 3 }`
(compile-time, provenance = this incident). A slice is zero-delta only when
ALL four deltas are zero for that slice. On the K-th consecutive zero slice:
emit `TerminateStalled` with the named reason + the last nonzero-delta
slice id; L2 treats it as terminal-finish (no retry, no restart).

**Field rollback (R5)**: one release behind env `ASTER_STALL_POLICY=off`
(default on), removed after one clean bench cycle.

**Acceptance**: (a) red→green fixture reproducing the freeze scenario
(latch + read-only + acceptance gate → must terminate ≤ 4 slices with the
named reason); (b) long-exploration fixture: 20 slices of pure reads with
genuinely novel targets → NOT terminated; (c) short-task latency corpus
unchanged (±5%); (d) terminal-bench read-only subset (fixture source: the
user's bench set, captured hermetically — see P4).

## Phase 3 — L2 + threshold provenance (de-fossilize)

**Touchpoints**: `unattended.rs` HostAdmissionBudget → AdmissionPolicy
consumer; migrate `50/256/32/16/840_000/1800` into
`Policy{value, source, reviewed}` structs (R3: compile-time; host layer may
only tighten via an explicit allowlist attribute never present in
workspace layers); lint: disallowed-bare-integers on the two files.

**Acceptance**: zero bare thresholds in the two files (rg-proven); every
Policy cites an issue/evidence path; unattended battery green.

## Phase 4 — Satisfiability CI gate + bench regression

**Touchpoints**: new test target `satisfiability_matrix`; dimension
registry (R4): a `RuleDimension` closed enum — every gate/ceiling enum in
fact_control/goal_control/unattended declares its member; adding a member
without a matrix row fails a meta-test.

**Matrix**: scripted-model micro fixtures (no network; scripted LLM
responses), state space = posture × gate-state × loop-state ×
progress-state; each cell must reach a terminal state within bounded steps.
CI budget: ≤ 90 s parallel.

**Bench fixtures**: capture the freeze-class tasks (terminal-bench4
read-only subset) from the user's bench set into
`tests/fixtures/progress-bench/` as hermetic replays.

## Ordering, risk, coordination

- P0 → P1 → P2 strictly; P3 parallel-safe; P4 last (repair-lane dependent).
- Concurrency: harness_loop/goal_control are outside the concurrent
  session's current WIP set (verified); re-verify at each phase start.
- Every phase: hermetic tests + evidence `docs/evidence/execution/<date>-<slug>/`
  + one-behavior-per-commit; constitution check re-run per phase.

## Explicitly not claimed

No loop merging; no L1/L4 cap changes; no PR#8 invariant change; no
runtime-tunable policies; no live-model claims.
