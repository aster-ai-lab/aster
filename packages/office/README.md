<a id="a3s-office-source-for-desktop"></a>

# A3S Office source for Aster

This private package is Aster's shared Office frontend. It contains document,
Markdown, spreadsheet, presentation and PDF editors, with React, Vue and web
component adapters. Studio consumes the built library, workers, fonts and WASM
assets from this package.

Run `just build-studio` from the repository root to build ACP, Office and the
desktop host together. For Office alone, run `bun run build` from this package.
The build and `bun run kernel:build` regenerate `generated/office-kernel.wasm`
from the root `crates/office` workspace when its sources or `Cargo.lock` are
newer. Install the `wasm32-unknown-unknown` target first. Packaging sets
`OFFICE_KERNEL_REQUIRED=1` and stops when that target is missing.

Run `bun run test` from this package for its owning tests. Browser harnesses and
A3S Test commands live in [tests/e2e/README.md](tests/e2e/README.md). Product chrome
follows the root [DESIGN.md](../../DESIGN.md); shared pickers and role tokens come
from the local `@a3s-lab/ui` package.

Document, Markdown, spreadsheet and presentation React editors are controlled:
pass accepted `onChange` content back through `content`. Pass the host's resolved
`theme` so Office follows its preference.
For a local spreadsheet, `onDerivedChange` receives recalculated results; retain
that content for saving without marking a user edit. Without that callback,
results still reach `onChange`. Collaboration retains its
existing shared calculation and history ownership.

`deps/xlsx-0.20.3.tgz` retains the patched SheetJS archive and its license.
The Aster pnpm workspace applies compatibility patches from `patches`.
`LICENSE` and `THIRD_PARTY_NOTICES.md` retain the upstream notices.
