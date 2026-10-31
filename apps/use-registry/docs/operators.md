# Registry operator procedures

Status: development preview. These procedures govern the **dev-preview
bootstrap root** published under `registry/`. They become the production
procedures only after the custody, rotation, expiry, withdrawal,
mirror-replacement, and recovery gates in the
[Aster Use roadmap](https://github.com/aster-ai-lab/Use/blob/main/ROADMAP.md) (A5)
pass review.

## Custody

- Signing seeds live outside every repository, in an operator-controlled
  custody directory (one `*.key` file per TUF role, `0600`).
- The four roles (`root`, `targets`, `snapshot`, `timestamp`) use independent
  Ed25519 keys, threshold 1 each.
- `keygen` refuses to overwrite existing key files; rotation is always an
  explicit, reviewed act.
- Back up seeds offline before the first publication. A lost targets key
  freezes the registry; a lost root key requires re-bootstrapping every
  consumer's `--trust-root` pin.

## Publication

```bash
# One-time per environment:
aster-use-registry-tools keygen --keys-dir <custody>/dev-preview

# Every publication:
aster-use-registry-tools assemble \
  --keys-dir <custody>/dev-preview \
  --admissions admissions.acl \
  --out-root registry \
  --metadata-version <previous + 1>
aster-use-registry-tools verify --registry registry \
  --expected-root-sha256 sha256:<pinned-digest>
```

- The root digest (`root.json` SHA-256) is the bootstrap identity. It changes
  only on root rotation; republishing targets bumps
  `--metadata-version`, not the root.
- Admissions are the review record: a package enters the tree only through a
  reviewed `admissions.acl` entry plus its committed package directory.
- The package manifest (`aster-use-extension.acl`) owns package identity;
  admission metadata owns presentation. Assembly fails closed when they
  disagree.

## Expiry

- Dev-preview defaults: root 365 days, top-level metadata 30 days.
- Refresh metadata before expiry by re-running `assemble` with the same keys
  and an incremented `--metadata-version`.
- Root expiry requires rotation (below).

## Rotation

1. Generate a new custody key set in a new directory.
2. Publish a root that adds the new root key and removes the old one (TUF
   root version `N+1`); the tooling for chained roots lands with the Use
   A5 gate — until then, rotation means re-bootstrap.
3. Re-bootstrap consumers: `registry source replace` with the newly obtained
   root digest. A redirect or rename never rotates trust.

## Withdrawal

Remove the admission block, bump `--metadata-version`, republish. Withdrawn
bytes stay in git history; the signed catalog stops advertising them. For
advisory metadata, set the catalog `availability` state through a reviewed
admission change (supported when the catalog tooling exposes it).

## Incident recovery

- **Tampered target bytes**: verification fails closed on digest mismatch;
  re-run `assemble` from the committed admissions to republish exact bytes
  under a new metadata version.
- **Lost snapshot/timestamp keys**: rotate by re-publishing all four roles
  from the surviving material under a new root (re-bootstrap path until
  chained-root tooling lands).
- **Committed tree unverifiable**: revert the publication commit; the
  previous tree remains the served truth.

## Local serve (transport only)

For a reproducible local HTTP source of the committed `registry/` tree, see
[local-serve.md](local-serve.md). Summary:

```bash
./scripts/serve_local.sh start
./scripts/smoke_local.sh
./scripts/consume_local.sh   # needs ASTER_USE_BIN with registry routes
./scripts/serve_local.sh status
```

Clients pin the same bootstrap root digest as production consumers and select
the local base URL with `--url`. Localhost is not a trust root. See
[local-serve.md](local-serve.md) for the package-manager vs capability-wrapper
distinction.

## Tooling source

`aster-use-registry-tools` is versioned by the
[Aster Use](https://github.com/aster-ai-lab/Use) repository (`crates/registry-tools`)
and must be built from a reviewed Use revision.
