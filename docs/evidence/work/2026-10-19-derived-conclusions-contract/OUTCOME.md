# Work source status and derived-conclusion contract

Date: 2026-10-19

## Result

Only two existing Work delivery-contract bullets were expanded:

- Source goals, targets and plans must not become completed events or actual
  periods. Assumptions are labeled “待核实” as unverified hypotheses; the answer
  must not imply supporting source evidence or confirmed causes.
- Every derived numerical conclusion must be checked with available tools,
  retaining the source inputs and formula. Comparisons use the same periods,
  coverage, units and exclusions; unmatched bases and invented causal
  explanations are excluded. Derived numbers omit thousands grouping separators
  unless the user or source explicitly requires that format.

Existing requested precision, original source spellings/quotations, ASCII signs,
missing-value treatment, file format, source preservation, deliverable scope,
and length checks remain in the contract.

The contract grew by **474 characters**, from 3138 to **3612** (trimmed). With
all 8000 caller characters and the two-character delimiter, it occupies
**11614**, below ACP's unchanged **12000**-character guidelines limit.

## Callers and boundaries

Work `turns.rs::host_for_session` calls `delivery_host`, which appends the
compile-time `delivery_contract.md`. `session_new_params` sends it through
`_meta.a3sHost.guidelines`. ACP `studio_host_from_meta` sanitizes and admits
that text, and `merge_studio_host` preserves it in `SystemPromptSlots.extra`;
`SystemPromptSlots::build` assembles the model system prompt.

The existing Work structural test now checks all new clauses in that exact
`session/new` metadata. The existing ACP transport/assembly test loads the same
Work Markdown, uses the full 8000-character caller allowance, parses the actual
metadata shape, merges prompt slots, and checks the built prompt retains the
complete contract once, beside caller text, review instructions and the budget.
ACP changes are entirely inside its existing test module. No ACP production
parser, tool behavior, Core or Studio production source was changed.

The `output_check` implementation was reviewed, including its schema, caller
registration and tests. Its existing bounded actions compute percentage changes
and text length; they do not determine source semantics, causal evidence,
comparison-basis equivalence or arbitrary derived ratios. This slice adds
instructions to use available tools, not an automatic semantic validator or
another tool API. Percentage-change requests retain the existing `output_check`
action; no output-check production change was needed.

## Exact incremental diff and SHA

[change.patch](change.patch) records only this slice against the pre-edit dirty
source snapshot: **3 files, 28 insertions and 7 deletions**. Existing shared-tree
changes are excluded. Baseline repository HEAD:
`e4344fc07d51503f3e65eae8b1d29e18c2bcfdaf`.

Patch SHA-256:
`16c3f704a0bf7dfbca01bd3d22e5e31977295d99d8ac5c9928d67120f13be0a9`.

| Final source | SHA-256 |
| --- | --- |
| `apps/work/src-tauri/src/agent_plane/delivery_contract.md` | `50034b870a76256619c3d35f38c164b8104bee51d4016d3b80aa9f43e638e6c0` |
| `apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs` | `bac14ae58bf20408942a655b55893a8e15757b8cbb5d48fedd6737826d5a2ef3` |
| `crates/code/acp/src/host_context.rs` | `7af593f09f9a789ab940c8988d75519c4019d60e71017434063f78c60fe27441` |

The four source/test SHA-256 values from the preceding Presenton contract slice
were independently compared and remain unchanged.

## RED and GREEN

Both runs used one serialized Cargo command for both owning packages, from the
repository root:

```text
cargo test --locked --offline -p aster-dream -p a3s-code-acp --lib --no-fail-fast -- delivery_contract host_text_lands_in_extra_and_keeps_the_budget_line
```

RED, fresh compilation, session 48307, **exit 101**:

- ACP: **0 passed, 1 failed, 314 filtered**.
- Work: **2 passed, 1 failed, 778 filtered**.
- Both failures named the missing rule against turning source goals/plans into
  completed events/actual periods. The contract had not yet been changed.
- Cargo reported 2m 36s to finish the test profile.

GREEN, after only the contract text changed, fresh compilation, session 47807,
**exit 0**:

- ACP: **1 passed, 0 failed, 314 filtered**.
- Work: **3 passed, 0 failed, 778 filtered**.
- Cargo reported 50.00s to finish the test profile.

Cargo produced these freshly compiled test executables:

```text
target/debug/deps/a3s_code_acp-fe795c66e1b39f0b
target/debug/deps/a3s_studio_lib-980ddfdb3c93364c
```

Both invocations waited for the shared target owner when necessary. No duplicate
Cargo invocation was started while either one remained active.

The scoped formatting and whitespace checks passed:

```text
rustfmt --edition 2024 --check apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs crates/code/acp/src/host_context.rs
git diff --check -- apps/work/src-tauri/src/agent_plane/delivery_contract.md apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs crates/code/acp/src/host_context.rs
```

## Explicitly not claimed

- The tests establish contract presence and transport, not model compliance,
  factual correctness of a later answer, or automatic semantic enforcement.
- No original-task input, answer, oracle, collected output, credential, ACL or
  user session configuration was read for this slice. No task number, answer,
  scoring vocabulary, source-specific numeric value, intent classifier or
  model-name heuristic was added.
- No formal task prompt, scorer, generated artifact or saved source was changed.
- No deployment, GUI action, live model replay or original-task pass was claimed.
  Current running/warm sessions are not claimed to refresh a compile-time
  contract merely because this Markdown source changed.
- The complete workspace was not rebuilt, retested or claimed formatted.
