---
name: registry-selftest
description: How to discover, review, install, and remove packages from the official A3S Use Registry.
---

# A3S Use Registry consumption

This Registry serves signed packages through static TUF metadata.
Every package is verified end to end before anything is installed. Use the
workflow below; never bypass the review boundary.

## Trust model

- Trust comes from the TUF root you pinned with `--trust-root`, never from
  the transport (GitHub, a mirror, or a private host).
- A pinned root digest is long-lived. Rotating it is an explicit, reviewed
  `registry source replace` with a newly obtained digest.
- Offline installs replay the same verified evidence; they never re-fetch
  weakened metadata.

## Lifecycle commands

```bash
# Configure the pinned source (digest obtained out of band):
a3s-use registry source add official \
  --github A3S-Lab/Use-Registry \
  --trust-root sha256:<pinned-root-digest> \
  --json

# Discover and inspect:
a3s-use plugin search <query> --scope-kind user --scope-id user/<name> --json
a3s-use plugin inspect <publisher>/<name> --scope-kind user --scope-id user/<name> --json

# Plan first, review the plan digest, then apply:
a3s-use plugin plan-install <publisher>/<name> \
  --scope-kind user --scope-id user/<name> --json
a3s-use plugin apply-plan --operation-id <id> --plan-digest <sha256> \
  --scope-kind user --scope-id user/<name> --yes --json

# Observe, cancel before admission, or remove:
a3s-use plugin observe-operation <publisher>/<name> --operation-id <id> \
  --plan-digest <sha256> --scope-kind user --scope-id user/<name> --json
a3s-use uninstall <publisher>/<name> --scope-kind user --scope-id user/<name> --json
```

## Rules that keep you safe

1. Planning never mutates state. Only `apply-plan` with the exact operation
   ID, plan digest, and explicit `--yes` performs the reviewed cutover.
2. An `Ask` plan is not consent. A human must confirm at the apply boundary.
3. Interrupted operations recover exactly: rerun the same command and the
   durable replay finishes the same plan without side effects.
4. Prefer `--json` output for automation; parse the typed fields instead of
   human-facing text.
