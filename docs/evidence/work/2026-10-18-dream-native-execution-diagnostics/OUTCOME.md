# AsterDream macOS command availability diagnostics

The reported `native_process_lifetime_unqualified` is a deliberate execution
refusal, not a filesystem permission failure. The native macOS adapter cannot
prove that every descendant is gone at settlement: its Unix process group does
not contain a child that calls `setsid` or `setpgid`. Full access does not supply
that missing lifecycle guarantee.

The refusal now tells the model that changing permissions or retrying the same
process tool cannot repair the condition. It directs artifact work to
in-process native Office/file tools. Default sandbox selection also retains
the guest discovery failure instead of discarding it. Missing environment
variables, unresolved artifacts, wrong file types, removed artifacts, and
unavailable HVF have explicit reasons. The refusal remains typed, native
policy digests remain unchanged, and no host-process fallback is introduced.

## Qualification gap

- This host reports `kern.hv_support: 1`, but neither
  `A3S_CODE_GUEST_SYSTEM_IMAGE` nor `A3S_CODE_KRUN_SHIM` was set during this
  investigation. Neither `target/debug/a3s-oci-krun-shim` nor
  `target/release/a3s-oci-krun-shim` existed.
- Core requires `a3s.oci.guest-command.v2`. The checked-in legacy producer at
  `crates/oci-runtime/crates/krun/src/macos_guest_command.rs` still reports v1
  and treats a VM worker exit as the payload exit. Downgrading Core to accept
  it would restore the known false-completion bug. Existing uncommitted v2
  compatibility changes were preserved.
- The current Work packaging paths do not install/configure that guest shim
  and system image for Bash. The earlier
  `docs/evidence/work/2026-10-18-guest-status-v2/OUTCOME.md` explicitly excludes
  complete macOS Bash integration and Core hardware guest E2E.
- Enabling macOS Bash safely requires integrating a producer that proves the
  v2 payload status, its immutable image and signed shim, desktop artifact
  discovery/packaging, and the existing Core HVF E2E covering exit status,
  workspace/network boundaries, escaped descendants, timeout, cancellation,
  and shutdown. The existing v2 producer qualification is evidence for that
  producer, not an automatic qualification of this desktop path.

## Validation

- `cargo test -p a3s-code-core --lib sandbox::guest -- --nocapture`: did not
  compile or execute tests (exit 101). Concurrent workspace manifest migration
  had `a3s-oci-sdk` resolving its `a3s-oci-core` dependency to missing
  `crates/core/Cargo.toml`. This change did not edit those manifests.
- `rustfmt --edition 2021 --check crates/code/core/src/sandbox/native.rs
  crates/code/core/src/sandbox/native/lifetime_tests.rs
  crates/code/core/src/sandbox/guest.rs
  crates/code/core/src/sandbox/guest/fail_closed_tests.rs
  crates/code/core/src/agent_api/session_sandbox.rs`: passed.
- `git diff --check --` for those five source files: passed.
- Regression coverage added for unavailable workspace/manifest/shim/type
  diagnostics, retaining the typed refusal, both native command APIs and
  preflight carrying the reason, preserved policy digest, and absent command
  side effects. The existing default-sandbox/Bash test now checks that the
  guest discovery reason reaches the tool output.

## Explicitly not claimed

The parent's later retry, `cargo test --locked --offline -p a3s-code-core --lib
sandbox::`, reached compilation but stopped in concurrently changed
`agent_api/run_admission/owner_lifetime_tests/protocol_cancel_tests.rs:69`
because `AgentProtocolHostError::runtime_code` does not exist. It still did
not execute the native-boundary tests. This supersedes the earlier manifest
failure as the latest attempted validation result, not as a passing test.

No new macOS command execution is admitted. No change to full-access policy,
sandbox permissions, native lifetime qualification, or process-host opt-in was
made. No VM was booted, and the new Rust tests were not yet executed because
workspace manifest loading failed before compilation. No old compiled test
binary was used as evidence for this source change.
