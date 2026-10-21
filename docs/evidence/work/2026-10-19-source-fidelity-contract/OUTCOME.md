# AsterDream source fidelity contract

The existing Work delivery contract now explicitly preserves quantity phrases
(the number with its original counting word or measurement unit), original
numeric spelling, explicit names and required expressions, including facts
retained in summaries and rewrites. The user's explicit request for that
particular conversion or change to the source units/expressions takes precedence.
Calculated results remain distinct from source facts. The existing file-task
bullet now limits final delivery to the requested files and worksheets, requires
checking the planned contents before publication, and excludes unrequested
source copies and intermediate drafts from final delivery. Existing length,
missing-information and file-format rules were not duplicated.

## Reach and checks

- Work's `agent_plane/turns.rs::host_for_session` appends the compile-time
  `delivery_contract.md` through `delivery_host`. `session_new_params` carries it
  in `_meta.a3sHost.guidelines`.
- ACP's `host_context::desktop_host_from_meta` admits that bounded text;
  `merge_desktop_host` puts it in `SystemPromptSlots.extra` as host instructions.
  Core's `SystemPromptSlots::build_with_style` includes the extra in the model's
  system prompt. Tool permissions and safety checks are unchanged.
- The Work test checks the admitted `session/new` payload retains both caller
  instructions and one complete contract, including the conversion exception
  and the requested-file/worksheet scope and pre-publication content checks.
  The existing ACP test now loads this same contract and checks it survives
  metadata parsing, prompt-slot merging and system-prompt assembly beside the
  session budget, caller text and review instructions.
- Work and Desktop have separate `turns.rs` implementations. Desktop does not
  currently inject Work's delivery contract. This change is limited to the
  requested AsterDream Work product; it adds no Core-wide or Desktop rule.
- Skills load on demand: Core's catalog exposes `search_skills`/`Skill`, and ACP
  local assets expose `open_local_asset`. A skill body is not an unconditional
  delivery-contract entry point.
- Existing `output_check` actions measure Unicode text length and calculate
  percentage changes. Missing and zero baselines return a missing/undefined
  result; percentage display uses ASCII signs and omits insignificant decimal
  zeros unless explicit user precision takes precedence. The delivery contract
  asks the model to use these checks; it does not automatically validate every
  final response or enforce semantic source fidelity.

## Validation

- `cargo test --locked --offline -p aster-dream --lib delivery_contract`:
  **3 passed, 0 failed**, on the final source below.
- `cargo test --locked --offline -p a3s-code-acp --lib host_context::tests`:
  **9 passed, 0 failed**, on the final source below. One preceding final-source
  attempt exited **101 before tests** because concurrently edited
  effort tests referenced an unimported `effort_limits_from_meta` in
  `agent_tests.rs:1681` and a nonexistent `ModelConfig.api_timeout_ms` in
  `agent/reasoning_transport_tests.rs:149`. The owning agent was notified;
  no unrelated production/test source was changed for this slice. The owner
  repaired those compile errors before the successful rerun.

Only the two changed Rust test files were formatted. The following checks passed:

```text
rustfmt --check --edition 2021 apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs crates/code/acp/src/host_context.rs
git diff --check -- apps/work/src-tauri/src/agent_plane/delivery_contract.md apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs crates/code/acp/src/host_context.rs docs/evidence/work/2026-10-19-source-fidelity-contract/OUTCOME.md
```

No workspace-wide formatting was run.

Final source SHA-256 before the focused runs:

```text
e05b5d3408dcb38a8d824ad8540dcd64b128978a22be023e9949edadf931f844  apps/work/src-tauri/src/agent_plane/delivery_contract.md
9129cc10bb4b12a48bb7b6f97075a9716d3b0c90dceab19657e43fd2d0745a22  apps/work/src-tauri/src/agent_plane/delivery_contract_tests.rs
1cc347bfde51f2e68be23cc38f2eac853d4560d5bdb1b44d6f4e4ecb76ec7ffe  crates/code/acp/src/host_context.rs
```

## Explicitly not claimed

- A professional reply or unchanged numerical meaning does not establish a
  strict evaluator pass when a required original expression was changed. No
  A03 or other corpus pass is claimed here.
- No task samples, answers, prior model outputs, credentials, ACL files or
  runtime configurations were read for this change. No grader or model artifact
  was modified. The contract and tests contain no task-specific answers.
- No deployment, packaged app rebuild, live UI/model run or strict-grader rerun was done.
  Standing host instructions are admitted at ACP session creation; a warm
  runtime currently does not refresh them merely because source text changed.
- The structural tests prove contract transport and assembly, not that every
  model output will obey it. No semantic regex, synonym table or answer matcher
  was introduced.
