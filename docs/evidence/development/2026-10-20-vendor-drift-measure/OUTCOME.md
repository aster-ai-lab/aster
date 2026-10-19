# First drift measurement: devices and vm-memory

## Result

Both names stay in the drift baseline. The byte difference is behavior,
so neither copy was overwritten.

`diff -rq` between the aster and cloud libkrun trees found no differing
file bodies. Each name differs by files that exist only on the aster
side:

- `devices`: `src/devices/build/windows_init_wrapper.rs`, a guest init
  wrapper that mounts, logs, and `fexecve`s an init binary.
- `vm-memory`: `.cargo/config` sets `+crt-static` rustflags for
  `aarch64-unknown-linux-musl`, and `.cargo/audit.toml` ignores two
  criterion-only RustSec advisories.

The api lineage has neither package. There is no third copy.

## Commands and counts

- `diff -rq` on the two `devices` directories — one "Only in" line, the
  `build` directory.
- `diff -rq` on the two `vm-memory` directories — one "Only in" line, the
  `.cargo` directory.
- Drift baseline unchanged: 11 names. No `--update-baseline`.

## Explicitly not claimed

No winner was chosen. No file was copied or deleted. `a3s-form-core`
and the later names in the reconcile order were not opened.
