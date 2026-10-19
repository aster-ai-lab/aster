# Host platform and Bash execution environment audit

Read-only source investigation for a later planning decision. No production
source, frozen configuration, model submission or runtime asset was changed.
No model, guest command, VM, test, application or background observer was
started. No model reasoning, current task body or submitted program was read.

## Finding

The current environment block describes the **host** using an unqualified
`Platform` label. For the macOS host with its configured OCI sandbox, Bash
executes in a **Linux guest** instead. The Bash definition does not supply
that distinction. This is a concrete description gap. It does not establish
that this gap caused any particular model command or failure.

### Environment construction and actual fact path

- `crates/code/core/src/agent/turn_context.rs:523–530` renders `<env>` from the
  workspace, `std::env::consts::OS`, `ARCH` and local date. The text labels the
  host value `Platform`, without saying that it is the host. On macOS that
  source produces `macos (aarch64)`.
- `turn_context.rs:544–546` returns no shell guidance on non-Windows hosts.
  The Windows branch at 534–540 supplies host PowerShell guidance.
- `turn_context.rs:447–460` appends this environment block to the augmented
  system prompt. It does not inspect the installed `BashSandbox`.
- Fact `LiveCompletion` enters `model_context` at `fact_control.rs:459` or
  `505`. `fact_control/model_call.rs:573–575` calls `fact_model_context`;
  `turn_context.rs:50–65` calls `prepare_turn_context`; its result at
  `159–162` uses the augmented-system builder above.
- `fact_control.rs:462–475` merges those extra sections into an admitted
  standing system. The assembled `system` is passed to `model_response`
  at `735–739`. Thus the normal, successfully assembled fact main turn has
  the host platform description, irrespective of its guest command backend.
- `fact_control/model_call.rs:580–586` can return no augmentation when context
  setup times out. This audit has not captured the actual provider wire and
  does not claim that every particular request received the environment block.

### Bash definition and actual executor

- `crates/code/core/src/tools/builtin/bash.rs:520–531` describes a shell in the
  workspace and specially describes Windows PowerShell. It gives no effective
  backend OS or shell for the macOS OCI configuration.
- `tools/types.rs:1160–1165` copies that static description into a
  `ToolDefinition`; `tools/registry.rs:331–339` exposes those definitions.
  The code profile also uses the same description in its compact catalog:
  `tools/presentation.rs:298–303` and `337–359`. Catalog descriptions are
  capped at 192 bytes, so essential execution context should not rely on a
  long suffix surviving that projection.
- `sandbox.rs:139–219` defines `BashSandbox` execution, admission, lifetime
  and policy methods, but currently has no method that reports the effective
  execution OS and shell to prompt assembly.
- `sandbox/oci/bundle.rs:129–130` launches
  `/bin/bash --noprofile --norc -c <command>`; `146–148` requires the OCI
  template's Linux configuration. On non-Windows hosts,
  `tools/builtin/bash.rs:1088–1095` passes command text unchanged.
- Static inspection of the selected frozen asset root found
  `target/t013-assets-hardware/bundle/rootfs/bin/sed` to be an absolute guest
  symlink to `/bin/busybox`; the guest `bin/busybox` file exists. An absolute
  guest symlink need not resolve through the host filesystem. This is not a
  missing-asset finding and does not imply that all Linux utilities are GNU.

### Delegated tool-capable children

`tools/task.rs:983–991` gives the child the parent's workspace services and
sandbox; `995–999` constructs its agent. The normal tool-capable path invokes
`ChildRun` at `1102–1109`, which enters `FactRun::open_projected_for` at
`agent_api/conversation_runtime/child_run.rs:66–74`. It therefore assembles the
same host-only platform block even though it inherited the guest sandbox.
The tool-free structured-generation branch at `tools/task.rs:1062–1073` is a
separate path; this finding does not assert identical prompt construction for
all child or auxiliary calls.

## Smallest general follow-up

1. Have the actually bound Bash backend supply a small typed execution-target
   description. The OCI implementation can authoritatively report Linux and
   `/bin/bash`; a custom backend that cannot report its environment remains
   unknown. Do not infer the target from the host OS, model command text or a
   filename. An additive default method on the existing sandbox contract is a
   candidate that preserves existing custom implementations; no new backend,
   configuration format or framework is required.
2. Render separate **host platform** and **default Bash execution target**
   fields from that bound description. Keep the host information for native
   file/GUI context. Keep explicitly approved host execution distinct. Do not
   globally replace the platform with Linux, label the tools GNU, broaden the
   sandbox, rewrite model commands or silently switch executors.
3. Supply the same authoritative target context to parent and inherited-child
   main calls under adaptive/direct/code presentation. Ensure its mandatory
   part survives the optional context-provider timeout and the compact-catalog
   description limit. The ordinary system/input digest evidence can bind the
   rendered text; this audit does not call for a new public event protocol.

This is a planning candidate only. The current frozen CUA evaluation continues
with its original source and configuration.

## Minimal general regression design (not executed)

- Use a fake bound backend declaring Linux Bash with a simulated macOS host.
  Capture a controlled provider request through the real fact main-call
  assembly. Assert separate host and command-target fields, with no generic
  platform label suggesting that Bash uses the host OS.
- Repeat at the tool-capable delegated-child boundary and under adaptive and
  code presentation. Assert that the child inherits the same target and that
  compact tool descriptions cannot drop the mandatory target information.
- Cover a host-execution backend and an unknown custom backend: report their
  actual/unknown targets without inventing Linux, GNU or BSD semantics.
- Exercise the existing optional context-timeout seam with a bounded fixture;
  mandatory execution-target information must remain accurate. Keep all
  existing permission, escalation and unavailable-backend refusals intact.

These checks need no real model, guest command or VM. A successful source-level
regression would prove the description contract, not that this single wording
change improves model correctness or whole-task latency.

## Explicitly not claimed

No causal attribution is made for the current model's syntax choice. No actual
provider request was inspected, no utility was run to probe its behavior, and
no performance improvement or task correctness gain was measured. All existing
frozen evaluation results and failed samples remain unchanged.
