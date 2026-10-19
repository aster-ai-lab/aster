# T028 execution-target context

Status: root captured real RED (0/2 passed, exit 101 in `red-2.json`) and
controlled-provider/context GREEN (15 actual tests passed; `green.json` and
`green-1.log` through `green-5.log`). The earlier `red.log` was a
workspace-manifest parse failure, not a regression run. `green-6.log` matched
zero tests and is retained without counting it as coverage.

## Scope

The existing `BashSandbox` and `WorkspaceCommandRunner` contracts gain an
additive, typed target-reporting method with an `Unknown` default. Existing
custom implementations remain source-compatible and their targets stay
unknown until reported. Known built-in backends report their configured
execution OS/shell; the host platform remains a separate fact.

The mandatory environment context is shared by normal fact-call assembly and
the existing optional-context timeout fallback. It is outside the compact tool
catalog. The Bash description and command schema point to that actual target.
Execution routing, permissions, confirmations, sandbox policy and command text
are unchanged. No configuration, dependency or runtime framework is added.

When no sandbox is bound, the renderer follows the current ToolContext route:
governed local default execution is unavailable without a configured sandbox;
other calls use the bound workspace command runner if present. Explicit
escalation describes that runner and requires the existing exact-invocation
permission and confirmation gates. It is never an automatic host fallback.

## Initial regression fixture

`execution_target.red.rs` is a frozen copy of the initial two-test fixture.
SHA-256: `0b9a71e2367a602e038b272b42f1c1baa001ec990c17ca0569e6c6d0ea7a18ee`.

Filter: `agent_api::tests::fact_log::execution_target::`

The fixture uses a controlled `LlmClient` through real `Session.send` and fact
model-input assembly. An unreported bound sandbox must not inherit the host
shell. A pending optional context provider is dropped by the existing 100 ms
assembly timeout, while the required host/target context must reach the model.
The fixture executes no command, model API or VM. Cargo has not been run by
this worker.

## Implemented contract and controlled verification

Known backends report host Bash, host PowerShell, Linux guest `/bin/bash`
(OCI), or Linux guest `/bin/sh` (GuestLifetime's supervisor contract).
PowerShell metadata reports no unobserved version. Unknown backend facts are
never replaced by the host. Target rendering performs no command execution,
asset discovery or shell lookup, and grants no authority.

The complete fact-provider filter now has four tests. Its two delegation tests
exercise adaptive `task` and code `program` → `task`, each with ordinary and
timed-out parent context assembly. They observe the actual parent/child/parent
provider requests and the child's inherited presentation/target, without
changing the intentional non-inheritance of optional context providers.

Seven pure context tests cover the closed set, synthetic macOS/Windows hosts
with a bound Linux target, plain local runner, governed local missing sandbox,
unknown sandbox over known local runner, unreported custom runner, and absent
runner. The existing two grounding/date checks are preserved.

Verified root-owned filters:

- `agent_api::tests::fact_log::execution_target::` (4 tests)
- `agent::turn_context::tests::execution_target::` (7 tests)
- `agent::turn_context::tests::env_block_` (2 tests)

The connected sandbox-availability fixture and governed missing-sandbox refusal
each passed one test. These ran from the frozen Core libtest at
`target/qualification/t028-cua-20261020/core-tests`, SHA-256
`cb8900f887acba37b8fc8fce05986c2c8d6035463a12cc04ed0c9a1c4d0cd06f`.
The provider filter took 1.30 s inside the test process; this is a hermetic
regression duration, not live-model task timing.

This worker ran pinned `rustfmt` directly on five owning files, finally using
the Core crate's edition 2021. No Cargo, live model, guest command, VM or GUI
was started by this worker. The root successfully compiled the implementation
and extended tests before running the frozen binary above. Later T005 work
added its own sibling test registration to shared `fact_log.rs`; the original
16-file freeze is preserved rather than rewritten.

The root then ran the combined T028/T005 Core artifact at
`target/qualification/t005-cua-20261020/core-tests`, SHA-256
`92486c70ac258c5e1c43b889818e0b00b3438ba27b7ef0cbd42551284db0e3a3`.
The same four fact-provider tests passed again in
[core-connected-07.log](../../goal-resumed-20261020/t005/core-connected-07.log).
The two previously missed portable Windows guidance cases each passed one
test:

- `tools::builtin::bash::tests::windows_shell_guidance_names_the_real_shell_and_its_chaining_rules`
  — [core-connected-08.log](../../goal-resumed-20261020/t005/core-connected-08.log).
- `tools::builtin::bash::tests::windows_host_shell_prefers_powershell_7_then_the_builtin_windows_powershell`
  — [core-connected-09.log](../../goal-resumed-20261020/t005/core-connected-09.log).

These are portable path/guidance unit tests running on the current macOS
host. They do not start PowerShell or establish actual Windows runner
qualification. The prior zero-match log remains retained. Across the initial
and combined artifacts, 17 distinct context/connected/guidance cases passed;
the repeated four-provider run is not counted as four additional cases.
The combined campaign's commands, binary identity and other connected proof
scopes are recorded in
[core-connected.json](../../goal-resumed-20261020/t005/core-connected.json).
Final scoped formatting, product builds and desktop evaluation remain
root-owned gates.

## Explicitly not claimed

No performance improvement, model correctness improvement, paid T01 result,
actual Windows/Linux execution qualification or GUI recovery qualification is
established by this context contract. Existing frozen task attempts and their
failed results are retained unchanged.
