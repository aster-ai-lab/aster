<a id="a3s-office-source-for-studio"></a>

# A3S Office source for AsterDream

This private package uses the same internalized Office source snapshot as
Aster Work. Installing AsterDream builds its editors, declarations, workers,
fonts, and WASM assets locally. Work keeps its separate frontend install.

`pnpm build` and `pnpm kernel:build` rebuild `generated/office-kernel.wasm`
from `crates/office` when those sources or `Cargo.lock` are newer. The command
is the AsterDream script, so both editor copies receive the same wasm. Install
the `wasm32-unknown-unknown` target first. Packaging sets
`OFFICE_KERNEL_REQUIRED=1` and stops when that target is missing.

`deps/xlsx-0.20.3.tgz` retains the patched SheetJS archive and its license.
The AsterDream pnpm workspace applies compatibility patches from `patches`.
`LICENSE` and `THIRD_PARTY_NOTICES.md` retain the upstream notices.
