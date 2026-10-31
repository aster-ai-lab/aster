# Aster Use Registry

The official signed package Registry deployment for
[Aster Use](https://github.com/aster-ai-lab/Use).

> **Status:** dev-preview bootstrap. The tree under `registry/` is signed by
> the development custody key set and is verified end to end by the released
> client, but it is **not** a production trust root. Production bootstrap
> material is added only after the custody, rotation, expiry, withdrawal,
> mirror-replacement, and recovery procedures in the
> [Aster Use roadmap](https://github.com/aster-ai-lab/Use/blob/main/ROADMAP.md) are
> implemented and reviewed.

## Bootstrap pin

| Item | Value |
| --- | --- |
| Bootstrap root SHA-256 | `sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172` |
| Root version | 1 |
| Metadata version | 3 |
| Served path | `registry/` on `bootstrap/dev-preview` (dev-preview custody) |

Clients consume this Registry through the pinned digest, never through the
transport. Use a Use **package-manager** build (`0.3.x+` with `registry`
routes). Homebrew capability wrappers (`0.1.x`) do not implement Registry
consumption.

```bash
# Prefer an explicit package-manager binary when PATH has a capability wrapper.
export ASTER_USE_BIN=/path/to/aster-use   # crates/use target/debug/aster-use

"$ASTER_USE_BIN" registry source add official \
  --github aster-ai-lab/Use-Registry \
  --trust-root sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172 \
  --json
```

Architecture (transport vs trust, ownership): 
[docs/registry-service-architecture.md](docs/registry-service-architecture.md).

## Admitted packages

| Package | Version | Surfaces |
| --- | --- | --- |
| `aster/registry-selftest` | 0.1.0 | Skill |
| `aster/applet-demo` | 0.1.0 | Tool, MCP, Skill, UI (`bind_tool=["echo"]`) — package source + signed `registry/targets/.../applet-demo` (Track A/J6/S1) |

## Ownership boundary

This repository owns reviewed admission records (`admissions.acl`), committed
package sources for admitted releases (`packages/`), and the signed static
publication state (`registry/`).

It does not own:

- the package manager, Registry formats, or authoring and verification tools;
  those belong to Aster Use (`crates/registry-tools` on Use `main`, binary
  `aster-use-registry-tools`);
- package source code for third-party packages; those stay in their owning
  repositories and are admitted as reviewed release artifacts;
- consumer installation authority; each Aster Use installation owns its selected
  source, independently obtained bootstrap-root digest, reviewed plan, Grants,
  and activation state;
- Desktop Applet UiHost projection or rendering; registry supply for Applet UI
  packages is tracked in
  [docs/applet-ui-package-supply.md](docs/applet-ui-package-supply.md)
  (monorepo plan:
  [desktop-applet-plugin-path](../docs/desktop-applet-plugin-path.md)).

## Trust model

GitHub is a transport and review surface, not a trust root. A client must pin
an independently obtained SHA-256 digest for the initial TUF root and must
verify the complete metadata chain and target digests before installation.
Repository renames, redirects, branches, and Git history do not grant trust
authority.

Operators: see [docs/operators.md](docs/operators.md) for custody,
publication, expiry, rotation, withdrawal, and incident procedures.

## Local serve (dev-preview transport)

To start a stable local HTTP transport for the committed `registry/` tree
(no Docker):

```bash
./scripts/serve_local.sh start
./scripts/smoke_local.sh
# optional: prove Use package-manager consume (needs ASTER_USE_BIN with registry routes)
./scripts/consume_local.sh
# first-principles gate (ephemeral port; needs ASTER_USE_BIN for I1/I5)
./scripts/test_local_registry.sh
```

Default base URL: `http://127.0.0.1:4873/`. Point a Use **package-manager**
client (`0.3.x+`) with `--url` and the same bootstrap `--trust-root` pin above.
Homebrew capability wrappers without `registry` routes cannot consume this
tree. Full recipe: [docs/local-serve.md](docs/local-serve.md).
