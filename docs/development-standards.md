# Aster Development Standards

Migrated from the Aster OS standards in the sibling `aster` repository and
restated for this workspace. The rules below describe how Aster is
built and verified. Each section states the rule, the rationale, and
the enforcement mechanism.

This repository has three Cargo workspaces and one constitution.
The regime of a change is the regime of its path. Crate names that
still start with `aster-` stay; they are API identifiers, not the
product name.

This is one of three governance surfaces, each with a distinct job:

- **This document** — the full rationale and enforcement mechanisms.
- [`.specify/memory/constitution.md`](../.specify/memory/constitution.md)
  — the enforceable distillation the spec-kit phases read. A
  NON-NEGOTIABLE mark may not be waived by any spec, plan, or task.
- [`AGENTS.md`](../AGENTS.md) — the directly actionable implementation and
  review rules, including Rust, cross-platform compatibility, local workspace
  boundaries, and protection from upstream binary replacement.

The spec-kit phase mapping lives in
[`docs/spec-driven-development.md`](spec-driven-development.md).

## Regimes

### Product regime

Paths: `apps/aster`, `apps/studio`, `crates/code`,
`crates/terminal`.

New behavior has a small hermetic test. Aster desktop tenants use
Tauri commands and stay in the root Cargo workspace. Members keep
`publish = false`.

### Control-plane regime

Paths: `apps/cloud`, `apps/api`, `apps/control-plane` (auth,
billing, config, runtime, runner).

A module keeps bounded contexts. A cloud migration bump includes
`CLOUD_MIGRATION_COUNT`. An HTTP surface uses `#[controller]` with
a guard and `auth.scopes`. A repository proof is a dual-adapter
proof against the in-memory adapter and live PostgreSQL.
`apps/api`, `apps/cloud`, and `apps/control-plane` keep their own
workspaces. Compat symlinks keep the legacy `apps/auth` family of
paths resolvable. `apps/gateway` remains a root member because the
desktop host embeds it.

## 1. Repository layout

**Rule.** Four Cargo workspaces live in this repository. The root
workspace serves the product paths (including the embedded gateway).
`apps/api`, `apps/cloud`, and `apps/control-plane` keep their own
workspaces and lockfiles, and they are not root members.

- `apps/aster` is the `aster` terminal binary. `crates/code/acp` is
  its `aster-acp` companion. Build and distribute both together.
- `crates/code/core` is the embeddable Rust SDK and harness. Do not
  add Node, Python, or Go bindings, SDKs, or package publishing jobs.
- `apps/studio` is the shared desktop host. Build it with
  `just build-studio`. Its default `aster` tenant is Aster.
  Keep tenant identities and data isolated. See `specs/004-desktop-default-aster`.
- Office uses the root `packages/office` and `crates/office` sources. The
  retired app is removed after migration. Do not copy
  old user state or add another Cargo workspace or lockfile.
- `crates/` holds Rust libraries, including internalized upstream
  trees. `packages/` holds TypeScript and React libraries (`ui`,
  `office`, and the other workspace packages).
- All members have `publish = false`. Internal `aster-*` dependencies,
  including optional ones, resolve to a path inside this repository.
  `tests/workspace_dependencies.py` guards that.

**Rationale.** A second workspace, lockfile, or published crate
becomes a second source of truth. The binary that ships must be the
tree people edit.

**Enforcement.** `AGENTS.md`, `tests/workspace_dependencies.py`, and
the root `Cargo.lock`. The user-authorized private desktop submodule at
`apps/studio` remains part of that workspace; its sources are committed only
to `git@github.com:aster-ai-lab/studio.git`. Other new remotes or sibling
checkouts created for Aster work violate principle XIII. A second checkout of
this same repository is a git worktree.

**Structural migrations commit whole.** A rename or relocation that
moves a tree must land as one commit. A partially staged crate (the
`Cargo.toml` without its `src/`) is a non-building intermediate.
Stage the complete migration, verify `git diff --cached --name-only`
covers every piece, then commit.

## 2. Language, toolchain, configuration

**Rule.** Rust on toolchain `1.97.1`. Build from the repository root
with the pinned toolchain. Product configuration is ACL (`.acl`),
parsed and generated only with `aster-acl`. Do not create, restore,
document, or parse another product configuration format.

**Rationale.** One configuration language with one parser keeps
resource limits and stable error codes. A second format drifts.

**Enforcement.** `AGENTS.md`. Host and workspace trust stay separated.
Sandbox enforcement and bounded reads stay in place.

**Extension pattern.** Per-transport settings live in nested ACL
blocks, not flat fields named after a transport. Adding a backend
extends a closed set plus its own parse arm. The selected provider's
transport block is required. Unknown or duplicate transport blocks
are refused.

## 3. Architecture by regime

**Rule.** Library code does not depend on a desktop shell. Cross-crate
data crosses the public Rust API of the owning crate. Preserve those
public APIs and the upstream package names unless a task explicitly
requires a rename.

On a product path, Aster desktop tenants stay in the root workspace.
On a control-plane path, each module is a bounded context with
`domain/`, `application/`, `infrastructure/`, `presentation/`, and
`published/`. Domain code does not import presentation or
infrastructure. `apps/api` and `apps/cloud` stay out of the root
member list.

**Rationale.** The receipt of an internalized library is its public
API. The control plane's receipt is the published fact, not a
private import. A second root member list for api or cloud would
merge three lockfiles.

**Enforcement.** Workspace membership, path dependencies, and review.
Shrink-only ratchets that already exist (the native-dropdown list,
dialog motion, the crate-drift baseline) may only shrink. Fix the
condition. Do not edit the allowlist to pass.

## 4. Error handling

**Rule.** Fail closed. Denied and missing share the not-found
contract where a caller must not learn which one occurred. An unset
required value refuses activation instead of defaulting. A
misconfigured optional surface degrades to disabled with an
error-level log. It does not take down the process that is still
able to serve.

**Refinements.**

- Idempotency is by fact identity. A replay is the same fact. Only a
  divergent owner or digest conflicts.
- Production paths do not call `unwrap()` or `expect()` unless a
  comment states why that construction cannot fail.

**Enforcement.** Review and the tests for the refusal. There is no
repository-wide unwrap census in this tree. A new production
`unwrap()` still needs the infallibility comment.

## 5. Naming

- Closed sets use an enum plus `parse()` / `as_str()`. The parse
  error names the set it refused.
- Crate and package identifiers that already start with `aster-` stay,
  including `aster-acl` and `aster-acp`. Do not rename them in
  passing.
- The default user-visible product name is Aster (desktop tenant branding remains explicit). Do
  not describe this product as Aster OS or as a Grok release.
- Files are snake_case. Rust modules are snake_case. Types are
  CamelCase. ACL attributes are snake_case.
- Example domains use `.example`. Do not reintroduce retired brand
  strings as if they were the product name.

## 6. Persistence

- Prefer the typed query API of the crate that owns the store.
  Values are bound parameters. SQL text that is fixed is
  `&'static str`.
- A raw query is allowed only when the typed builder cannot express
  it. Document that reason at the call site. Keep values
  parameterized.
- Schema changes stay inside the crate or app that owns them.
- On a product path, the change stays with that owner. Ordinary
  desktop or CLI work does not require a live PostgreSQL campaign.
- On a control-plane path, cloud migrations stay a contiguous chain.
  Registering one bumps `CLOUD_MIGRATION_COUNT` and
  `LATEST_CLOUD_MIGRATION_VERSION`. A store with two adapters proves
  the same scenario on both.

## 7. Commands and controllers

On a product path, Aster desktop tenants expose behavior through
Tauri commands.

- A new command is registered on the host and allowed by a
  permission that the calling window's capability includes. Applet
  windows do not receive host permissions they do not need.
- Command inputs are typed. Unknown fields are rejected at the
  boundary that parses them.

On a control-plane path, an HTTP surface uses `#[controller]` with
a guard and `#[metadata("auth.scopes", ...)]`. Body DTOs reject
unknown fields. A new metadata key is a platform decision.

Tests for a command or a controller live in a sibling module. Do
not append thousands of inline test lines to a production file.

## 8. Testing

- On a product path, new behavior gets a small hermetic test.
  Prefer `cargo test` for the owning package, or the app's existing
  Vitest project, over a live external service.
- On a control-plane path, a repository or authority proof is a
  dual-adapter proof. PostgreSQL harnesses use
  `ASTER_CLOUD_TEST_POSTGRES_URL` and `--test-threads=1`.
- Whole-file test modules start with `#![cfg(test)]`.
- After a run with many failures, re-run each distinct failure alone
  before treating it as a product defect. Parallel runs can fail
  because of shared fixtures or another session's files.
- A failed assertion on an error message is triaged before it is
  "fixed". The implementation that refuses with a precise reason is
  often right. The expected wording is what drifts.
- Every landing that changes behavior records
  `docs/evidence/<domain>/<date-slug>/OUTCOME.md` with the commands,
  the counts, and an Explicitly not claimed section. Evidence
  directories are not deleted to make a tree look tidy.

## 9. Security

- Compare bearer secrets in constant time.
- A foreign resource is indistinguishable from a missing one when
  the caller must not learn that it exists.
- No hardcoded credentials. Do not copy user session state into the
  repository.
- Fail closed on security surfaces. Degrade, with a signal, on
  convenience surfaces.
- Updates must not replace Aster with an Aster or Grok release.
- Preserve host and workspace trust boundaries and sandbox
  enforcement.

## 10. Git and delivery

- Stage files individually. Do not `git add -A` or sweep a directory
  another session is editing. Verify `git diff --cached
  --name-only` immediately before committing.
- The exception is a migration's own new tree: a relocation commits
  whole (§1).
- Commit subjects are imperative. Write the message with `-F` when
  the body contains backticks.
- Do not commit unless the user asked.
- A slice that changes behavior lands with its tests and, when the
  landing is recorded, its `OUTCOME.md`.

### Cross-platform text and checkout

- `.gitattributes` owns line endings on Windows, Linux and macOS; do not
  depend on a contributor's global `core.autocrlf`. Source, ACL, JSON,
  Markdown, shell and PowerShell text use UTF-8 and LF. Windows `.bat`
  and `.cmd` files use CRLF in the checkout and canonical LF in the index.
- Generate text with an explicit encoding and newline policy. Use UTF-8
  without a BOM by default. A required legacy encoding or BOM is an explicit,
  documented runtime/fixture contract and needs a focused compatibility check.
- Evidence, model fixtures and other byte-identified inputs keep their exact
  bytes through explicit `-text` attributes. Do not normalize frozen records
  to satisfy a formatter or whitespace check. Scope any whitespace exception
  to the immutable records that require it.
- Before staging across Windows/WSL, inspect `git check-attr text eol -- PATH`
  and the staged diff. An unexpected whole-file replacement requires checking
  line endings before proceeding. Review `git diff --cached --check` and
  `git diff --cached --stat`. Renormalize only individually reviewed paths;
  never run repository-wide renormalization over another session's edits.
- Keep case-sensitive paths, executable modes, native path separators and
  shell quoting explicit at platform boundaries. Check behavior on each
  claimed target; a Windows executable run through WSL interop is not a
  native Linux qualification.

**Enforcement.** `.gitattributes` is exercised by
`python tests/line_endings.py`: real Git add/checkout under both `autocrlf`
settings must preserve UTF-8, LF scripts, CRLF command files and immutable
bytes. The line-ending workflow runs it on Windows, Linux and macOS. A small
fixture is not a full-history renormalization or a substitute for native E2E.

## 11. Documentation

- `OUTCOME.md` states the result, what landed, and what is explicitly
  not claimed. The last section is mandatory.
- Record abandoned alternatives when a decision would otherwise be
  re-derived.
- A dangling reference is a defect. When a document moves, the same
  change updates every pointer to it.
- `DESIGN.md` owns product chrome. This document owns engineering
  rules. Do not add a second design spec.

## 12. Governance

- A rule change lands here with the evidence that the rule was
  followed, then distills into the constitution. One amendment
  path, two views.
- A NON-NEGOTIABLE principle cannot be waived by a spec, a plan, a
  task, or a deadline. The only move is a constitution amendment
  with evidence.
- Independent judgment and adversarial review are verified in
  practice, not by a scanner.

## 13. Names are not guessed

**Rule.** A name, entity, or alias comes from an explicit source:
the model writes it in an entity list, the author writes it inside
quotes, or the string is already stored and a later query matches
that string exactly.

Do not infer a name from capitalization, a character class, a lead
character, a verb list, a stop list, a prefix or suffix ratio, or a
regular expression tuned to one example sentence. When a sentence
is missed, leave it missed.

Lexical tokens used only to rank text, secret redaction by a known
token prefix, and a short closed product word list that classifies
a sentence already stored are not name extraction. Those lists do
not grow to cover one missed sentence.

**Enforcement.** Constitution principle XIV. Review rejects a new
matcher of this kind.

## 14. Meaning is not matched

**Rule.** A decision about what the user means — a request to
remember, a preference, an intent, an importance judgment — is made
by the model or by a typed closed set the host declared. A regex, a
substring, a verb list, or a character class must not make that
decision.

Pattern matching may narrow fail-closed (secret redaction,
credential refusal), rank candidates for the model, or parse a
mechanical form. It may not classify intent. A missed phrasing uses
the model path. Matcher vocabularies do not grow to cover it.

**Enforcement.** Constitution principle XV.

## 15. Pickers are drawn

**Rule.** A product dropdown is drawn by the design system. Native
`<select>`, `<datalist>`, and `document.createElement('select')` do
not ship. Use `SearchSelect` from `@aster-ai-lab/ui`, `Combobox` when the
trigger must read as a text field, or the same anatomy drawn where
React is not available. The contract is `DESIGN.md` §6 "Picker
(dropdown)". The Radix `Select` list only shrinks.

**Enforcement.** Constitution principle XVI.
`apps/studio/src/renderer/__tests__/noNativeSelect.test.ts` fails on
a native dropdown in Aster's renderer and holds the Radix `Select`
list shrink-only. Component tests open the trigger and choose a
`role="option"` row.

## 16. Dialogs stay in place

**Rule.** A modal dialog, including a desktop side panel with
`role="dialog"`, appears and disappears where it rests. It fades
and scales in place. It does not slide in from outside the window,
and its motion does not animate `transform` or `filter`. An
anchored menu may rise at most 0.5rem. That rise composes with the
positioning transform and does not set `filter`. A phone-width
bottom sheet is the only edge travel. Durations and the rest of
the contract are `DESIGN.md` §6 "Dialogs".

The existing class names `.aster-dialog-scrim`, `.aster-dialog-surface`,
`.aster-popup-rise`, `.aster-row-enter`, and `.aster-row-leave` stay.
They are implementation identifiers.

**Enforcement.** Constitution principle XVII.
`packages/ui/src/components/primitives/__tests__/dialog.test.tsx`
and `popup-motion.test.tsx` lock the in-place dialog and popup
motion.

Transient menus and popovers also follow `DESIGN.md` §6 "Transient popups": outside
presses dismiss without consuming the clicked action or stealing its focus. The shared UI
primitives own this behavior, including nested portals and application focus changes.
`packages/ui/src/components/primitives/__tests__/popup-dismissal.test.tsx` checks it.
Boolean controls reuse the shared neutral `Switch` instead of page-specific implementations.

## 17. Rust implementation and review

**Reference.** Microsoft's [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/),
version 2026.6, consulted 2026-10-29. This section adapts selected guidance
to Aster; it does not claim that the existing tree satisfies every item.

**Scope.** Apply these rules to new and materially changed handwritten Rust
in each path's regime. Review existing debt explicitly. Preserve public APIs,
the workspace split, ACL authority, and source provenance. Generated and
vendored trees keep their source-of-truth workflow; do not sweep them for style.
The actionable rules are collected in `AGENTS.md` under "Rust implementation
and review" and "Cross-platform compatibility"; this section explains them.
OS-sensitive changes use the existing platform adapters, preserve release
baselines, and record native verification for affected targets. See §10 for
the cross-platform text and checkout contract.

### Diagnostics and lint exceptions

Use the pinned compiler, rustfmt, and Clippy (`M-STATIC-VERIFICATION`). For
the owning package, run `cargo clippy --locked -p <package> --all-targets --no-deps`
with its supported features. Choose features deliberately; `--all-features`
is not a substitute for a valid build configuration. Keep failures and their
scope visible. New findings are fixed or recorded with an exact follow-up.

Prefer item-scoped `#[expect(lint, reason = "...")]` when the lint is
expected in that configuration (`M-LINT-OVERRIDE-EXPECT`). A target-dependent
`#[allow]` needs a reason. Do not silence an entire crate or enlarge the
workspace allow list to hide one finding. A workspace lint table only applies
to members that opt in through `[lints] workspace = true`; verify inheritance.

Public types should support useful `Debug` output. Credential-bearing types
use a redacted implementation, tested with synthetic secrets, including nested
configuration and header values (`M-PUBLIC-DEBUG`). Keep sensitive values out of
`Display` and telemetry too. Use the existing `tracing` stack with stable fields;
reserve stdout for the CLI or protocol's intended output (`M-LOG-STRUCTURED`,
`M-LOG-NOT-PRINT`). See [Universal](https://microsoft.github.io/rust-guidelines/guidelines/universal/).

### Errors, types, and public contracts

Recoverable external failures return the owning library's typed errors; retain
the source when converting between error layers. Callers match types or stable
codes, not rendered messages. Application boundaries may use the existing
`anyhow` stack and add actionable, non-sensitive context (`M-APP-ERROR`). Aster
keeps its published error enums; adopting `M-ERRORS-CANONICAL-STRUCTS` does not
authorize an API-breaking conversion. See [Applications](https://microsoft.github.io/rust-guidelines/guidelines/apps/).

Use `Path`/`PathBuf`, enums, and invariant-bearing types where they express the
contract. Validate a newtype in its fallible constructor and keep fields that
could invalidate it private. Builders validate cross-field constraints at
`build()`; do not revalidate an already admitted value downstream
(`M-STRONG-TYPES`, `M-STRONG-TYPES-GUARD`, `M-BUILD-RESULT`). Prefer explicit
public re-exports. Unit tests stay in sibling files; public API integration
tests belong in `tests/`. Test-only bypasses are absent from normal builds
(`M-NO-GLOB-REEXPORTS`, `M-INTEGRATION-TESTS`, `M-TEST-UTIL`). See
[Resilience](https://microsoft.github.io/rust-guidelines/guidelines/libs/resilience/).

Public module docs explain purpose and observable effects. Public functions
document applicable error, panic, cancellation, and safety obligations, with
small examples when useful (`M-MODULE-DOCS`, `M-CANONICAL-DOCS`). Do not add
empty sections or comments that restate the code. See
[Documentation](https://microsoft.github.io/rust-guidelines/guidelines/docs/).

### Unsafe, async, and performance

Prefer safe Rust and established platform wrappers. Every handwritten unsafe
block or impl has a nearby `SAFETY:` explanation of the actual pointer,
ownership, lifetime, or thread-safety guarantees. Unsafe APIs document their
caller's obligations under `# Safety`. Do not use unsafe to evade lifetimes or
`Send`/`Sync` requirements. Performance-motivated unsafe requires measurements
(`M-UNSAFE`, `M-UNSOUND`). Expected input, I/O, and cancellation failures are
not panics; §4's construction-level argument still governs `unwrap`/`expect`
(`M-PANIC-IS-STOP`). See [Correctness](https://microsoft.github.io/rust-guidelines/guidelines/correctness/).

Bound input bytes and work at the entry boundary. Use async I/O, or isolate
blocking I/O and long CPU work with the runtime's existing blocking mechanism.
Do not hold a blocking mutex guard across `.await`. Long async computations
yield in bounded chunks and retain a defined cancellation path. Moving work
to `spawn_blocking` does not make a running closure abortable. Measure before
changing allocators, hashers, ownership, or collection representations
(`M-YIELD-POINTS`, `M-HOTPATH`, `M-MEM-REUSE`). See
[Performance](https://microsoft.github.io/rust-guidelines/guidelines/performance/).

**Adoption limits.** These references do not authorize a crate-layout migration,
allocator switch, CPU-baseline increase, public API rewrite, or new toolchain.
Do not add audit, feature-matrix, dependency, or Miri tooling as part of a
routine standards edit. Such work follows the existing scoped change process.

**Enforcement.** Use existing format, compiler, Clippy, and hermetic test checks,
then read the affected control flow. Search matches are candidates, not proofs:
exclude generated code and identify test-only paths before reporting a defect.
Record confirmed findings, disproven candidates, command results, and coverage
limits in the change's existing `docs/evidence/` record. Do not claim a
repository-wide audit from a sample or a successful lint command.
