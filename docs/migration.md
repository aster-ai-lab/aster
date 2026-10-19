# Harness migration

The 2026-10-18 import copies the required Rust source snapshots from
`../a3s` into a standalone Aster workspace. The OS/backend checkout remains separate; the later Studio cleanup is
recorded below. Package-level provenance is recorded in
[`source-manifest.json`](source-manifest.json). Local snapshots include
working-tree changes; repository revisions do not imply release certification.

## Product and dependency boundaries

`aster` directly runs the existing pager composition root. It starts the
sibling `a3s-code-acp` process, which owns the Code Core harness. There is
no umbrella CLI or second agent loop. The old A3S umbrella CLI, cloud,
gateway, other desktop apps, web sites, distribution installers, and Code's
Node/Python/Go SDKs are outside this product.

The imported support crates cover the harness fact log, ACL parser,
native sandbox, memory, lexical retrieval, search/browser contracts,
extension reconciliation, OKF, optional workflows, and typed decisions.
Terminal support remains in its upstream directory structure to retain
resource paths, test fixtures, protocol definitions, and third-party notices.
The PTY controller and harness remain available for terminal regression tests.

All imported Rust packages and Work are members of one Cargo workspace with resolver 3,
shared dependency declarations, one checked-in `Cargo.lock`, one build
directory, and Rust 1.97.1. This follows the
[Cargo workspace model](https://doc.rust-lang.org/cargo/reference/workspaces.html).
The two default members build the terminal and ACP companion together.
Retained package names and Rust module paths keep the embeddable SDK stable.

## Compatibility decisions

- **Flow:** Code 9.1.4 uses Flow 1.1 contracts removed in the sibling's
  Flow 2.0 tree. The exact Code-pinned Flow commit
  `2948ad51a1395177764766c3ddf7e44338f9e374` is copied from the existing
  Cargo source cache. It is a local workspace dependency. Flow's optional
  Boot, Event, and ORM integrations now resolve to in-tree sources too.
- **Small support libraries:** Common 0.1.1, Lane 0.5.1, and Updater 0.2.0
  have no source project in the sibling tree. Their exact cached crate
  sources are included locally, with original licenses and provenance.
  Updater is retained for the standalone Search package; Aster's own
  automatic and explicit binary-update entry points are disabled.
- **ACL:** all imported consumers use the sibling's ACL 0.3.0 locally.
  Host-authoritative configuration remains protected from repository
  overrides. Workspace MCP trust and sandbox checks remain in place.
- **SQLite:** ACP and Memory move from rusqlite 0.32 to 0.37, matching
  the terminal crates, because Cargo permits only one `sqlite3` native
  linkage. Both memory adapters use sqlite-vec 0.1.7-alpha.10. Existing
  database schemas and migrations remain intact.
- **Runtime:** Tokio Util requires at least 0.7.19 for cancellation-token
  identity comparisons already used by Core.
- **Vector test utilities:** Vec's old exact tempfile 3.14 dev pin is
  widened to 3.27 to share the workspace's temporary-file library.
- **Configuration and state:** existing `.a3s` / `.a3s-code` paths and
  `A3S_*` environment variables remain compatible. The migration copies
  no personal credentials, session stores, browser profiles, or build outputs.
- **macOS native execution:** the source Core deliberately refuses native
  shell execution with `native_process_lifetime_unqualified` until descendant
  lifetime and settlement are qualified. This guard remains intact. The two
  shell-dependent PTY success tests are skipped only on macOS; native refusal
  tests cover the guard. Launching, model chat, and file tools remain available.
- **SDK-independent fixtures:** evaluation/research wire fixtures move
  from `sdk/` into `crates/code/fixtures/protocol`; Rust protocol tests
  point at the new paths. No foreign-language bindings are retained.
- **Release tests:** Search's standalone upstream release-workflow test is
  retired with that release pipeline. Root workspace CI and Aster's
  migration tests replace the obsolete publication-layout assumptions.
- **Memory tests:** the legacy ACP `v912_remember_before_stream_close`
  oracle expects a raw-input fallback and JSON files even when the extraction
  model returns no memories. Current Core explicitly forbids that fallback
  and uses Vec snapshots. The historical test remains ignored with its reason;
  Core's current memory-retention tests verify the supported contract.
- **Hardware and linking:** retain Apple Silicon's 16 KiB jemalloc page
  calibration, Linux ARM64 page settings, static MSVC CRT settings, and
  musl linker hardening. Python-extension dynamic lookup flags are removed.

## Licensing

The terminal and Aster entry point retain Apache-2.0 licensing. Code Core,
ACP, and the supporting A3S libraries retain their original MIT licenses.
Third-party notices remain under `crates/terminal`. The root Apache license
covers the product entry point and does not relicense imported MIT code.

## Validation

Validation results are recorded in [`validation.md`](validation.md).
Real-model qualification, external browser installation, optional cloud
backends, and release certification are separate from the local migration.

## Local launch commands

`just code` builds both default workspace members and runs the current
`aster` TUI. Arguments are forwarded as positional shell arguments.

`just work` runs `just build-work`, then starts
`{cargo metadata target_directory}/debug/aster-dream`. A failed build does
not launch. It replaces an older running primary AsterDream, including one
under `apps/work/src-tauri/target` or the primary Application Support app,
so the new build is not asked to show the old window. `just dev` starts the
same desktop through `apps/work`'s launcher with Rsbuild dev on
127.0.0.1:4320, so renderer edits hot-reload. Studio keeps 4318. Work is included in
the same Rust workspace. Its former Git metadata is preserved locally under
`.git/imported-work.git`; its source files are part of this repository.

The welcome screen uses the supplied Aster logo, retained at
`assets/aster-logo.png`, with full and compact ASCII artwork. Forty-eight
frames are cached lazily and replayed at 12 FPS. Glyph and color sweeps
share the existing welcome-screen redraw clock. Compact artwork reserves
five rows so a standard 80-by-24 terminal can still show it. Color output
uses the terminal's existing quantization and NO_COLOR handling.


## Complete internalization

The workspace now contains 137 Rust packages. A3S dependencies in every
manifest and the lockfile resolve to local source, including optional,
target-specific, and test dependencies. Every workspace member is marked
`publish = false`. The additional source snapshots are recorded in
[`internalized-sources.json`](internalized-sources.json).

Work shares ACL, OKF, Vec, Use Core, and reconciliation with Code. OCR,
Use Extension, Tool Acquire, Boot (including macros), Event 0.3, ORM, and
Office's four native/kernel crates are included. The pinned OpenAI client
fork and Work's modified PTY adapter are local as well. OCR's image dependency
uses the workspace-compatible 0.25.10 release.

Code, Work, and Studio share the Flow 2 engine at `crates/flow`, with
package name `a3s-flow` and library identity `a3s_flow`. The initial import
retained Flow 1 separately for Code at commit
`2948ad51a1395177764766c3ddf7e44338f9e374`. The root workspace now resolves
one engine, `a3s-flow` 2.0.0, from `crates/flow`. `crates/flow-v2` is not a
workspace member. The workflow authoring package stays under
`crates/flow/packages/ui`.

Work's desktop copies of the shared Rust libraries and its separate lockfile
are removed. Its renderer stays in a pnpm workspace. Office's full editor
source, build configuration, and checked-in WASM kernel replace the old
artifact-only dependency; `pnpm install` builds the editor locally. Work's
source packages are private and standalone package publication scripts are
removed. Its default tenant assets and formerly external test fixture are
included. Source builds, development launchers, macOS packaging, and the
Linux container build use the root Cargo workspace and target directory.

Work remains AGPL-3.0 under `apps/work/LICENSE`; the Rust libraries and third-
party forks retain their respective original licenses.

## Studio import

Studio is imported from `../a3s/apps/studio`, including its local source
changes. Provenance is recorded in `source-manifest.json`. It stays at
`apps/studio` alongside Aster Work. The host (`a3s-studio`), native database
runner, and lightweight desktop contract tests join the root Cargo workspace.
There is one Cargo lockfile and target directory; the default members stay
Aster and ACP. All three imported packages have `publish = false`.

Studio shares local ACL 0.3, Boot, Flow 2, OKF, OCR, Vec, and Use libraries.
The duplicate ACL authoring dependency is consolidated into the local parser.
ORM gains the upstream engine catalog and statement-grid APIs. Use Extension
gains the Paperloom host mode and composer-mode contract. These additions
preserve existing public APIs. Stale Studio memory diagnostics and tests use
the current personal/private scope contract; room-memory isolation remains.

Frontend libraries live under `packages`: UI, Intelligent UI, Form, CE, and
an independently installable copy of the internalized Office source and kernel.
Flow UI stays under `crates/flow/packages/ui`. `apps/web` contains only the
shared Flow designer, supporting components, tenant assets, and an AIX fixture.
Registry source packages under `apps/use-registry/packages` supply renderer
imports and native fixtures; no web or cloud service is imported.

Install the frontend in `apps/studio` with `pnpm install --frozen-lockfile`.
Run `just build-studio` at the root to build ACP and Office, then Studio.
`just studio` starts the development window. On macOS,
`just build-studio-release` packages the frontend, host, and companions.
Launch and packaging scripts resolve custom target directories through Cargo.
Studio keeps its upstream package, binary, tenant, and application identifiers.
Existing user data is not copied or migrated.

Studio retains AGPL-3.0 under `apps/studio/LICENSE`; libraries retain their
original licenses and notices. Standalone publication scripts and release
workflows are excluded. No credentials, local environment secrets, sessions,
Git metadata, build caches, or compiled companion binaries are imported.

The Studio renderer builds successfully. Its full TypeScript check still
reports upstream errors (324 diagnostics locally, compared with 349 in the
source checkout); every distinct local error also appears upstream. CI gates
the Studio renderer build and launcher tests while retaining Work's existing
type-check gate. Fixing unrelated frontend typing is outside this import.

After validation, `../a3s/apps/studio`, `../a3s/packages/intelligent-ui`, and
`../a3s/packages/intelligent-ui-aix` are removed from the source tree and moved
to the local Trash for recovery. OS, API, Gateway, and shared UI/Office/Form/CE/
Flow sources remain there. Studio's old submodule entry and launch commands
are removed from the OS checkout. Four backend live tests now read the identical
PDF fixture at `../a3s/fixtures/paperloom/two-pages.pdf`, so removing Studio does
not remove their input. Offline frontend installation and Rust checks pass
again with the source Studio directory absent.

## macOS command lifetime follow-up

On 2026-10-18, Aster adopted the owner-bound Guest adapter from Code commit
`cd3764c9` and the host-availability provider from `cb5b5521`. The paired OCI
Runtime shim is source commit `3189636cb`; it watches both the direct Core
process and the shim that owns the VM worker. The adapter and shim must be
deployed together. Exclusive command-script creation also rejects existing
files and links without overwriting them.

[Retained execution evidence](evidence/execution/2026-10-18-macos-guest-owner/OUTCOME.md)
binds the exact local sources and signed fixture artifact. Native macOS Shell
remains refused; product packaging, policy composition, control-file recovery,
network mediation, background streaming and the Python environment remain open.
No additional language SDK or in-process code sandbox was introduced.

## Backend services import

A second 2026-10-18 import brings the A3S OS backend into this workspace as
five new members: `apps/api`, `apps/auth`, `apps/billing`, `apps/config`, and
`apps/cloud/crates/sdk`, cut from `../a3s` at revision
`cde6ae1e0a1b0ee36078e12cf5defeeb814715d4`. Provenance and per-tree hashes are
recorded in [`source-manifest.json`](source-manifest.json) under
`backend_import`.

- Dependencies resolve locally: the four service manifests replace the
  `A3S-Lab/Boot` and `A3S-Lab/ORM` git pins with `crates/boot` (0.2.0) and
  `crates/orm` (0.3.1), the same versions the pins named. The Use extension
  path follows this repository's flattened `crates/use/extension` layout.
- Member `Cargo.lock` files are dropped; the root lockfile is the only one.
- Rust sources are reformatted to the workspace's rustfmt defaults. A
  whitespace-insensitive audit attributed every differing source file to that
  reformatting; the only content change is the removal of the `maas-leave`
  remote agent, deleted together with the a3s-maas service in the source
  repository.
- `apps/use-registry` and `apps/web` are imported as content trees (no Cargo
  membership); caches, build outputs, and local state are excluded.
- Validation: `cargo check --tests` on the five members, workspace
  `cargo fmt --check`, and a clean-room boot of all four processes against the
  live middleware on fresh databases — `/health` fully up, admin login through
  the auth directory, and a notes round-trip. See
  [`source-manifest.json`](source-manifest.json) `backend_import.verification`.
- Inherited debt: a subset of the api's in-source test campaigns fail at the
  source revision too (one overflows its stack, one architecture ratchet is
  red); they are recorded here rather than fixed, to keep the copy faithful.

## Cloud control-plane import

The same day, the cloud dependency closure lands: `crates/box` (the sandbox
runtime workspace: core, runtime, netproxy, plus the vendored deps and
guest sources that stay unregistered), `crates/runtime`, `crates/sms`,
`crates/form-core` (from the pinned A3S-Lab/UI source), `crates/oci-sdk`
and `crates/oci-runtime` (from the pinned A3S-Lab/OCI-Runtime source), the
`a3s-use` umbrella package, and the cloud members `apps/cloud/crates/
{contracts,node-agent}` joining the already-present sdk.

- `apps/cloud/crates/control-plane` is copied but not registered: its HEAD
  depends on the in-flight use-core lane (PluginSurfaceKind and the
  PluginSurfaceRef narrowing) that no published use-core carries yet. It
  registers once that lane lands.
- Version pins carry over as local paths: Boot/ORM/ACL/Use resolve to the
  in-tree crates; Box 3.2.7, Runtime 0.8.0, and UI form-core resolve to the
  copied trees. The box runtime keeps axum 0.6 next to the workspace's 0.8
  (its scale API predates the 0.7 rename), and use's rusqlite aligns to 0.37
  for the single native sqlite3 linkage.
- Validation: `cargo check` on contracts, node-agent, runtime, sms,
  form-core, and box-runtime, plus workspace-wide `cargo fmt --check`.

## Gateway, runner, and the full-module state

`apps/gateway` registers with the bge embedding crate and the vendored
updater and sentry sources; `apps/runner` registers. The science, web,
and official content trees are no longer in this checkout. crates/vec loosens its rayon pins
from exact 1.10.0 to ^1.10 so the gateway's image/ravif chain resolves
beside the vec index pin wall. apps/runtime (the Gateway /v1 data-plane
binary) shares the a3s-runtime package name with the copied exec
runtime and stays a standalone tree pending its rename decision. The
registry-builder tool ships unregistered against the pinned extension
0.1.3 for the same reason. Validation: vec 459, runner 33, gateway
compile-plus-tests, and the terminal product still compile.
