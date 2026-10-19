# A3S Flow Roadmap

Aster's consumers share this one engine under `crates/flow`. Local migration
checks and the remaining baseline failure are recorded in the
[single-engine outcome](../../docs/evidence/flow/2026-10-18-single-engine/OUTCOME.md).

**Status as of 2026-09-10.** Updated **2026-09-23** for OS Wave H (AX-inspired
Agent platform) non-goals only — no change to Flow's authority.

This repository owns deterministic workflow compilation, append-only history,
and worker-independent replay. Product availability for WaaS, Operations, and
Runtime CI/CD remains an A3S Cloud gate decision.

## A3S Cloud substrate obligations

| Priority | This repository must deliver | Forbidden |
| --- | --- | --- |
| `F0` / Operations | Durable history and receipts Cloud Operations already compose | Becoming desired-state or Agent transcript authority |
| `CD0` | Stage history primitives for Delivery Pipelines (source→build→release→promote→rollback) | Parallel Cloud updater or second workflow engine |
| `W0` | Heterogeneous adapter replay with owner side-effect identities | Product retry tables as truth |
| Wave H / `A1` · `AR0` | Recoverable multi-step / child-execution coordination when Cloud composes Agent trees through Flow + Operations | Owning `agent_execution_events`, approvals, checkpoints, egress policy, or a Harness run store; importing AX/Redis task queues |

Portfolio detail:
[coordination-and-data-planes.md](https://github.com/A3S-Lab/Cloud/blob/main/docs/project-roadmaps/coordination-and-data-planes.md),
[flow-execution-integration-roadmap.md](https://github.com/A3S-Lab/Cloud/blob/main/docs/flow-execution-integration-roadmap.md),
[architecture-optimization-roadmap.md](https://github.com/A3S-Lab/Cloud/blob/main/docs/architecture-optimization-roadmap.md).

OS Wave H (borrow AX invariants, do not fork AX):
[`docs/ax-inspired-agent-platform-plan.md`](../../docs/ax-inspired-agent-platform-plan.md).

Monorepo index:
[cloud-substrate-dependency-roadmap.md](https://github.com/A3S-Lab/a3s/blob/main/docs/cloud-substrate-dependency-roadmap.md).

## Local product work

Kernel, authoring, CLI, and Skill delivery continue in this repository’s
README and website. Local milestones are not Cloud availability claims.
