# Prune reference contract

The unchanged mise v2026.10.25 executable passes 12 fixed native cases and
89 actual commands. `tests/cli_mise_prune_reference.py` retains the inputs,
expected installed versions and original 25-second per-command deadline.
The accepted capture is frozen at
`specs/031-cli-mise-replacement/contracts/mise-prune-reference.json`.
The reference executable's SHA-256 is unchanged after the capture.

Cases cover an unreferenced current version, a global exact declaration,
two separately tracked projects, prefix selection, ordered versions,
changed/deleted declarations, both preview modes, unavailable confirmation,
configuration-only cleanup and a live native executable. Every retained
fixture executable is actually run and checked against its version. The
running executable remains alive through prune and its owned PID is reaped.

Command:

```sh
python3 tests/cli_mise_prune_reference.py --mise .tmp/mise-replacement/reference/mise --output .tmp/mise-prune/reference-fourth.json
python3 -m py_compile tests/cli_mise_prune_reference.py
```

The first two attempts fail during fixture setup: copying the macOS system
file flags requires a privilege, and the host Python 3.9 Path.write_text API
does not accept newline. Copying bytes and setting the test mode explicitly
fix both setup issues. The third attempt passes 11 cases, but its copied
system executable dies before the running-version assertion. A separate
native probe observes signal 9 for the plain copy and successful execution
after ad hoc signing of that same owned copy. The fixture now signs its copy
on macOS. The 12 case inputs, expected retention and deadlines are unchanged;
all rejected captures and logs remain retained.

The current actual Aster CLI rejects `tool prune --dry-run --json` with exit 2
in an isolated home and project. Its executable hash and exact output are in
`aster-admission.json`. The source has no prune command, and the store's
existing prune primitive preserves only the current pointer. This is a
verified implementation gap, not a passing Aster differential result.

The pinned source contract requires resolving tracked configurations and
retaining running tool versions. Existing Aster configuration loaders promise
no writes; an explicit CLI/store tracking operation must preserve that API.
See the research and still-unchecked T004 entry for the next implementation.

## Explicitly not claimed

- Aster prune, upgrade or complete C02/T004 implementation.
- Stub, lock/snapshot, exclusion and retargeted-link acceptance.
- Native Windows/Linux behavior or release qualification.
- A mise runtime dependency, published package or release.
