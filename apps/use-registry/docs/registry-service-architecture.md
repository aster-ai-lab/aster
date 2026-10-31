# Registry service architecture

Status: development preview  
Audience: operators and Use client integrators

## Executive decision

The Aster Use Registry **service** is a **signed static publication** plus an
optional **dumb HTTP(S) transport**. It is not a package manager, not a trust
root, and not a mutable control plane.

```text
admissions + packages ──assemble/sign──► registry/ (immutable tree)
                                              │
                         transport only       │
                    ┌─────────────────────────┼─────────────────────────┐
                    ▼                         ▼                         ▼
              GitHub raw                local serve              other mirror
                    │                         │                         │
                    └─────────────────────────┼─────────────────────────┘
                                              ▼
                                    aster-use package manager
                         pin trust-root → TUF verify → plan → receipt
```

Clients must pin an independently obtained bootstrap root digest. Changing the
URL, host, or mirror never changes trust.

## Layer map (best practice)

| Layer | What it is | Authority? | Where |
| --- | --- | --- | --- |
| **Publication** | Signed TUF metadata + target bytes | Yes (content) | `registry/` in this repo |
| **Transport** | Bytes over HTTP(S) / GitHub raw | **No** | `scripts/serve_local.sh`, CDN, raw.githubusercontent.com |
| **Client pin** | Host-selected `--trust-root` | Yes (bootstrap) | Use `registries.acl` |
| **Client cache** | Verified target digests | **No** (optimization) | Use `state/remote-registries/.../verified-targets` |
| **Installed package** | Immutable generation + receipt | Yes (install) | Use `data/extensions` + `state/extensions` |

Recovery and enablement never treat transport or verified-target cache as
authority. That separation is intentional and must not be collapsed into a
“pluggable storage” abstraction that mixes blobs and control state.

## Ownership

| Owner | Owns | Must not own |
| --- | --- | --- |
| This repository | Admissions, package sources for admitted releases, signed `registry/` tree, local transport scripts | TUF client, Grants, install lifecycle |
| Aster Use (`aster-use` package manager + `aster-use-registry-tools`) | Source config, TUF refresh, plan/apply, receipts, cache policy; assemble/sign/verify of Registry trees (`crates/registry-tools` on Use main) | Re-implementing a second Registry publisher inside Cloud or Code |
| Cloud / product hosts | Selecting sources and trust roots for a deployment | Embedding Docker/OCI registries as Use TUF registries |

`aster-use` **capability wrappers** (Homebrew `0.1.x` browser/box/office routes)
are not Registry clients. Only the Use package-manager binary (`registry` /
`plugin` routes, `0.3.x+`) consumes this service.

## Local registry service

Dev-preview local service = static file server of `registry/` with honest
readiness:

1. Recorded PID is alive.
2. That PID listens on the configured port (when `lsof` is available).
3. `GET {base}metadata/root.json` succeeds.
4. Smoke proves served root digest equals the bootstrap pin and on-disk tree.
5. Consume proves `plugin plan-install` provenance carries the same URL and root.

Foreign listeners and stale PID files must not count as ready. Wrong
`--trust-root` must fail closed at plan time.

Operator recipes: [local-serve.md](local-serve.md).  
Custody and publication: [operators.md](operators.md).

## What this architecture refuses

- Treating localhost, GitHub, or any URL as a trust root.
- Implementing TUF inside Cloud as a parallel Registry.
- Serving mutable “latest” catalogs without signed metadata.
- Using Registry cache as install/recovery authority.
- Long-term dual authority models for pre-release disk layouts.

Production bootstrap, rotation, and custody gates remain on the Use A5
roadmap; this document describes the service shape those gates must preserve.
