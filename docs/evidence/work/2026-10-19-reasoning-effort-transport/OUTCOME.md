# Explicit reasoning effort reaches the local ACP turn

Status: focused regression checks passed. Source frozen for the root agent's
next reviewed product build; this slice did not build or install a live binary.

## Confirmed defect and bounded repair

The renderer already sends `AiStreamOpenRequest.reasoningEffort` through both
`IpcChatTransport` and `useAgentChatRuntimeState`. Work and Studio local native
turns discarded it: `TurnWire` retained no selection and runtime setup applied
only the execution mode. ACP consequently kept its existing initial `high`.

Both native hosts now validate and retain the explicit selection, then use the
existing `session/set_model` request with `_meta.reasoningEffort` before the
model prompt. They send the existing exact provider/model ID, without guessing
its meaning. The same setting is applied after the existing process-restart
retry and before native workbench remediation. Unchanged accepted selections
do not rebuild the ACP session again.

ACP model/effort replacement resets the session posture. The host invalidates
its mode cache and reapplies the original execution mode before any prompt.
Failed configuration requests leave caches unknown and prevent the prompt.
An explicit rejected effort does not silently continue under the old effort.

The local host closed set is `default`, `none`, `minimal`, `low`, `medium`,
`high`, `xhigh`, and `max`. Omission and JSON null preserve the running
selection and do not send an effort reconfiguration. Explicit `default`
clears ACP overrides, matching the renderer's documented default sentinel;
the configured model's defaults still apply. ACP's existing `ultracode`
contract remains available to its existing clients, but the desktop's `ultra`
is not treated as an alias. Unsupported local values, including `auto` and
`ultra`, fail before storing messages or launching a local agent. Remote,
cloud, and form executors keep their own reasoning contracts.

No provider/model-name matcher, capability policy, default interactive effort,
public Rust API, credentials, or account configuration was changed or read.
The renderer's richer `ReasoningWireProfile` remains a separate contract; this
slice forwards existing effort values instead of interpreting its operations.

## Red before production repair

The new tests were added and run before the production repair:

```text
cargo test -p aster-dream --lib reasoning_transport_tests -- --nocapture
exit 101: 1 passed, 3 failed, 0 ignored
```

The peer received `high/auto` after an explicit `low`; a peer rejection did not
prevent the prompt because no setting request was sent; and unsupported
values created a turn instead of refusing admission. The absent/null baseline
passed.

```text
cargo test -p a3s-code-acp --lib reasoning_transport_tests -- --nocapture
exit 101: 0 passed, 2 failed, 0 ignored
```

Explicit `default` retained the previous `none`; unsupported metadata was
accepted rather than refused.

## Final hermetic checks

All final commands ran from the repository root with the pinned toolchain:

```text
cargo test -p a3s-code-acp --lib reasoning_transport_tests -- --nocapture
exit 0: 3 passed, 0 failed, 0 ignored

cargo test -p aster-dream --lib reasoning_transport_tests -- --nocapture
exit 0: 6 passed, 0 failed, 0 ignored

cargo test -p a3s-studio --lib reasoning_transport_tests -- --nocapture
exit 0: 6 passed, 0 failed, 0 ignored
```

The host tests exercise the actual local turn driver against an isolated ACP
peer. They verify every supported literal, ordering before the prompt, stable
process reuse, duplicate suppression, mode replay after replacement, null and
omission both initially and after a selection, backend refusal without a
prompt, cache invalidation after a rejected switch, and selection propagation
after the driver's existing restart retry. Unknown values are refused before
message rows or a process exist. The peer is an existing Node fixture style;
missing Node fails the tests instead of silently skipping.

The ACP tests use real session creation/replacement. A loopback HTTP fixture
serves bounded Anthropic responses; no live model endpoint is contacted.
Four completed turns (`high` → `low` → `none` → `default`) keep the same
session ID. Old replaced handles close and the new handle remains usable.
Every subsequent primary wire request contains all earlier user markers and
assistant replies; the durable transcript retains them too. Permissions are
reapplied to `auto` before each generation. Invalid effort does not replace
the current session or change the previous selection.

This fixture also checks existing adapter semantics rather than assuming
that the UI token disables thinking: `none` produces `thinking.type=adaptive`
and `output_config.effort=low` on the current Anthropic adapter. `default`
removes these fields for this fixture, whose model config does not declare
reasoning. Other configured model defaults can still enable reasoning.

While developing the fixture, one compile attempt caught a removed test-only
import and a nonexistent `ModelConfig.api_timeout_ms` field; both were fixed.
The first provider fixture only returned JSON and then wrongly required every
request to stream. It was corrected to serve the actual streaming primary
requests and bounded non-streaming auxiliary calls, with LLM memory extraction
disabled in the fixture. The final single history/wire case and full three-case
ACP filter both passed after these corrections. No earlier failed attempt is
counted as final evidence.

Owned-file `rustfmt --edition 2021/2024 --config skip_children=true --check`
and `git diff --check` passed. `cargo fmt --all -- --check` was invoked and
reported format drift in existing ACP engineering/goal/agent test files and
other shared-worktree sources outside this slice; these were not reformatted
as part of this change. Existing linker and dead-code warnings did not fail
the focused builds.

## Source identity and delivery

The exact eleven Rust sources, including the sibling tests, are listed in
[`source-sha256.txt`](source-sha256.txt). The aggregate SHA-256 of sorted UTF-8
relative paths, a NUL, complete file bytes, and a NUL is:

```text
1179a35f3eb5228a1db851c97b5d3730972003a2a98a26b9bda868c575c9a8d0
```

Production owners: Work and Studio `agent_plane/turns.rs`,
`workbench_turns.rs`, and new private `reasoning_effort.rs`; ACP `agent.rs`
and `agent/models.rs`. Each host has a new sibling
`reasoning_transport_tests.rs`, as does ACP's `agent` module.

The next product adoption needs a reviewed ACP and host rebuild. No running
binary, process, current task, model choice, user configuration, or deployment
was changed by this slice. Shared-worktree changes from other sessions were
preserved.

## Explicitly not claimed

- No live task score, speedup, latency bound, or successful A07 rerun.
- No claim that `none` shuts off provider thinking; the tested current
  Anthropic behavior is adaptive low, and GLM has its existing contract.
- No new model capability inference, `ReasoningWireProfile` interpreter,
  provider dialect policy, or mapping from `auto`/`ultra` to another token.
- No change to generic ACP control-RPC timeout/cancellation policy, which is
  shared with the existing execution-mode request path.
- No full-workspace green, product bundle result, installation, or deployment.
