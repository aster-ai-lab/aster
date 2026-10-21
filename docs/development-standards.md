# Aster Development Standards

Migrated from the A3S OS standards in the sibling `a3s` repository and
restated for this workspace. The rules below describe how Aster is
built and verified. Each section states the rule, the rationale, and
the enforcement mechanism.

This repository has three Cargo workspaces and one constitution.
The regime of a change is the regime of its path. Crate names that
still start with `a3s-` stay; they are API identifiers, not the
product name.

This is one of three governance surfaces, each with a distinct job:

- **This document** — the full rationale and enforcement mechanisms.
- [`.specify/memory/constitution.md`](../.specify/memory/constitution.md)
  — the enforceable distillation the spec-kit phases read. A
  NON-NEGOTIABLE mark may not be waived by any spec, plan, or task.
- [`Agents.md`](../Agents.md) — the quick reference for this
  workspace, including what must stay local and what must not be
  replaced by an upstream binary.

The spec-kit phase mapping lives in
[`docs/spec-driven-development.md`](spec-driven-development.md).

## Regimes

### Product regime

Paths: `apps/aster`, `apps/work`, `apps/desktop`, `crates/code`,
`crates/terminal`.

New behavior has a small hermetic test. AsterDream desktop tenants use
Tauri commands and stay in the root Cargo workspace. Members keep
`publish = false`.

### Control-plane regime

Paths: `apps/cloud`, `apps/api`, `apps/auth`, `apps/billing`,
`apps/config`.

A module keeps bounded contexts. A cloud migration bump includes
`CLOUD_MIGRATION_COUNT`. An HTTP surface uses `#[controller]` with
a guard and `auth.scopes`. A repository proof is a dual-adapter
proof against the in-memory adapter and live PostgreSQL.
`apps/api` and `apps/cloud` keep their own workspaces.

## 1. Repository layout

**Rule.** Three Cargo workspaces live in this repository. The root
workspace serves the product paths. `apps/api` and `apps/cloud` keep
their own workspaces and lockfiles, and they are not root members.

- `apps/aster` is the `aster` terminal binary. `crates/code/acp` is
  its `a3s-code-acp` companion. Build and distribute both together.
- `crates/code/core` is the embeddable Rust SDK and harness. Do not
  add Node, Python, or Go bindings, SDKs, or package publishing jobs.
- `apps/desktop` is the shared desktop host. Build it with
  `just build-desktop`. Its default `aster` tenant is AsterDream.
  Keep tenant identities and data isolated. See `specs/004-desktop-default-aster`.
- `apps/work` retains legacy AsterDream source and the Office vendor
  dependency. `just build-work` remains a legacy build. Do not copy
  old user state or add another Cargo workspace or lockfile.
- `crates/` holds Rust libraries, including internalized upstream
  trees. `packages/` holds TypeScript and React libraries (`ui`,
  `office`, and the other workspace packages).
- All members have `publish = false`. Internal `a3s-*` dependencies,
  including optional ones, resolve to a path inside this repository.
  `tests/workspace_dependencies.py` guards that.

**Rationale.** A second workspace, lockfile, or published crate
becomes a second source of truth. The binary that ships must be the
tree people edit.

**Enforcement.** `Agents.md`, `tests/workspace_dependencies.py`, and
the root `Cargo.lock`. A new remote or a sibling checkout created for
Aster work is a violation of principle XIII. A second checkout of
this same repository is a git worktree.

**Structural migrations commit whole.** A rename or relocation that
moves a tree must land as one commit. A partially staged crate (the
`Cargo.toml` without its `src/`) is a non-building intermediate.
Stage the complete migration, verify `git diff --cached --name-only`
covers every piece, then commit.

## 2. Language, toolchain, configuration

**Rule.** Rust on toolchain `1.97.1`. Build from the repository root
with the pinned toolchain. Product configuration is ACL (`.acl`),
parsed and generated only with `a3s-acl`. Do not create, restore,
document, or parse another product configuration format.

**Rationale.** One configuration language with one parser keeps
resource limits and stable error codes. A second format drifts.

**Enforcement.** `Agents.md`. Host and workspace trust stay separated.
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

On a product path, AsterDream desktop tenants stay in the root workspace.
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
- Crate and package identifiers that already start with `a3s-` stay,
  including `a3s-acl` and `a3s-code-acp`. Do not rename them in
  passing.
- User-visible product names are Aster and AsterDream (desktop tenant branding remains explicit). Do
  not describe this product as A3S OS or as a Grok release.
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

On a product path, AsterDream desktop tenants expose behavior through
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
  `A3S_CLOUD_TEST_POSTGRES_URL` and `--test-threads=1`.
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
- Updates must not replace Aster with an A3S or Grok release.
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
not ship. Use `SearchSelect` from `@a3s-lab/ui`, `Combobox` when the
trigger must read as a text field, or the same anatomy drawn where
React is not available. The contract is `DESIGN.md` §6 "Picker
(dropdown)". The Radix `Select` list only shrinks.

**Enforcement.** Constitution principle XVI.
`apps/desktop/src/renderer/__tests__/noNativeSelect.test.ts` fails on
a native dropdown in AsterDream's renderer and holds the Radix `Select`
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

The existing class names `.a3s-dialog-scrim`, `.a3s-dialog-surface`,
`.a3s-popup-rise`, `.a3s-row-enter`, and `.a3s-row-leave` stay.
They are implementation identifiers.

**Enforcement.** Constitution principle XVII.
`packages/ui/src/components/primitives/__tests__/dialog.test.tsx`
and `popup-motion.test.tsx` lock the in-place dialog and popup
motion.
