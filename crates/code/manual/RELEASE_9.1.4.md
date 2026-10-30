# 9.1.4 workspace integration

`a3s-code-core` and `a3s-code-acp` retain upstream version 9.1.4 and their
public Rust identities. In this repository all members have `publish = false`;
they resolve locally and are not registry-publishing targets.

## Delivery

- Rust hosts link `crates/code/core` through a local path.
- `aster` and `a3s-code-acp` are built and distributed together.
- Studio development and package recipes also build Office, with tenant
  identity and data isolation owned by the host.
- No foreign-language SDK, bridge package or publishing workflow is delivered.

## Qualification

Build from the root with the pinned Rust toolchain. Run focused tests for changed
behavior, the workspace dependency guard, formatting and applicable CI checks.
Record source/submodule state, features, actual binary hashes and residual failures.
A prior upstream release or a local debug pass does not qualify a later product
build or another platform. See [the integration guide](USER_GUIDE.md),
[release profiles](RELEASE_PROFILES.md) and [capability verification](CAPABILITY_VERIFICATION.md).
