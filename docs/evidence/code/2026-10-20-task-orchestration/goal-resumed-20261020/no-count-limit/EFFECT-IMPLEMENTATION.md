# Effect optional count limits

Status: Effect production and seven focused tests are frozen for root-owned
verification. This agent did not run Cargo, models, VMs, or GUI actions.

Owned source is limited to `crates/effect/src/{actor,coding,compose,lib}.rs`
and `crates/effect/tests/unbounded_counts.rs`. Existing T005 receipt,
duplicate-guard, and reconciliation changes remain present. No dependency,
manifest, configuration, task, or runtime asset was edited.

## Contract

`HarnessConfig` privately retains the real tool allowance and actor transition
ceiling as `Option<u32>`. Public constructors and existing numeric function
signatures are unchanged:

- `HarnessConfig::new(0, ..., positive_steps, ...)` still means zero tool
  allowance. It does not mean unbounded.
- A zero step limit or zero model-attempt count is still refused by that
  constructor. `with_step_limit(Some(0))` also refuses it.
- New `tool_budget_limit()` / `step_limit_option()` expose the actual options.
  The existing `budget()` / `step_limit()` getters keep their `u32` signatures;
  a missing ceiling projects to zero and is documented as a legacy projection.
  Execution paths must use the optional getters to distinguish `None` from
  `Some(0)`. No large-number surrogate is used for no limit.
- `with_tool_budget_limit(None)` removes only tool allowance and the automatic
  tools-off cap. `with_step_limit(None)` removes only the actor step ceiling.
  The two setters can independently restore an explicit positive bound.
  `without_count_limits()` removes both counts and the tools-off cap.
- Model retry attempts, compaction, duplicate protection, tool preparation,
  receipts, cancellation, and permission-bearing service boundaries are kept.

Actor `resume_with_optional_limit` / `ingest_with_optional_limit` and the
corresponding coding helpers use the existing execution loop. Existing APIs
delegate with `Some(limit)`, retaining the old `limit=0` non-executing peek.
The new `None` path skips only the transition-count comparison. It does not
race or detach effects, reorder preparation and commit, create another owner,
or replace cancellation. Actor step and tool-result counters saturate instead
of wrapping after the old count ceiling is removed.

The public `MetaHarnessSpec` struct shape and its numeric fields remain intact.
`HarnessGraph::from_spec` retains the original numeric allowance semantics.
Additive `from_spec_with_optional_budget` and `optional_budget` let actual
unbounded schedulers expose `CodingView::tool_budget=None`. `coding_actor`
passes the true option rather than reconstructing an allowance from the
legacy zero projection.

Core owns the separate configuration, capability, standing-digest, composed
recipe, and default FactRun drive changes. Its explicit `resume_limit(0)`
must remain a peek. A composed recipe's explicit positive tool/step override
must remain binding independently of an inherited unbounded setting.

## Focused tests

`tests/unbounded_counts.rs` contains seven deterministic, in-memory cases:

1. Constructors distinguish zero allowance from none and reject zero steps.
2. A finite controlled completion script executes 128 tools and more than
   200 actor transitions, retaining its tools and producing its actual final
   response without `budget.denied`.
3. Cancellation after 64 tools settles a cancelled receipt and interrupted
   response without executing the remaining scripted tools.
4. Explicit zero and positive tool allowances still deny the next tool while
   the actor transition count is unbounded.
5. An explicit step limit prevents the next preparation when tool allowance
   is unbounded.
6. Existing and optional `Some(0)` resume entry points remain non-executing.
7. Numeric Meta Harness recipes retain their bound; optional graphs expose
   true `None` and continue beyond 50.

Root-owned focused verification command:

```text
cargo test --locked --offline -p a3s-effect --test unbounded_counts -- --nocapture
```

Existing `harness`, `tool_round_cap`, `tool_effects`, actor, and compose tests
remain relevant compatibility coverage. The root agent also ran a separate
Core existing-API RED: an explicit zero Session option was actually rejected
before the Core integration repair; Effect's new APIs compiled in that graph.
That Core result is in [core-red.log](core-red.log). It is not an Effect
behavioral RED, and this document does not relabel it as one.

[effect-source-freeze.json](effect-source-freeze.json) records all five file
hashes. [effect-static-checks.json](effect-static-checks.json) records exit 0
for scoped Rust 2021 formatting and `git diff --check`.
An independent read-only agent reviewed the four production files and seven
cases and reported no static blocker; it did not compile or execute them.

## Explicitly not claimed

No Effect Cargo RED or GREEN result has been produced by this agent. Test
source and static review do not establish a passing execution result.
This Effect slice alone does not prove the host configuration path, composed
Core kernel, capability narrowing, standing binding, actual provider, or real
guest behavior. It does not remove timeout, cancellation, permission,
confirmation, verification, concurrency, or provider quota controls. No paid
task was retried and the original R2 evaluation remains unchanged.
