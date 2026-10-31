# Control-plane edge workspace

OS edge services that used to sit in the root Cargo workspace:

| Package | Path | Binary |
| --- | --- | --- |
| `aster-auth` | `auth/` | human proof |
| `aster-billing` | `billing/` | usage and invoices |
| `aster-config` | `config/` | distributed config |
| `aster-gateway-runtime` | `runtime/` | Gateway `/v1` data plane |
| `aster-runner` | `runner/` | self-hosted pipeline runner |

This directory is its own Cargo workspace and lockfile. It is not a
root workspace member. Build and test with:

```bash
cargo check --locked --manifest-path apps/control-plane/Cargo.toml --workspace
cargo test --locked --manifest-path apps/control-plane/Cargo.toml -p aster-auth
```

Compat symlinks at `apps/auth`, `apps/billing`, `apps/config`,
`apps/runtime`, and `apps/runner` keep legacy scripts, Tilt, and
submodule path strings working. Prefer the `apps/control-plane/...`
paths in new code.

`apps/gateway` stays in the **root** workspace because Studio embeds
it. `apps/api` and `apps/cloud` remain their own submodule workspaces.
