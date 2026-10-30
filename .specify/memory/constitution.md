# Aster Constitution

Distilled from the engineering practice of this repository. The full
rationale lives in
[`docs/development-standards.md`](../../docs/development-standards.md).
This document is the enforceable distillation the spec-kit phases
read. A principle marked **NON-NEGOTIABLE** may not be waived by any
spec, plan, or task.

One constitution covers this repository. A change follows the regime
of the path it edits. Crate identifiers that start with `a3s-` remain.

## Regimes

### Product regime

Paths: `apps/aster`, `apps/studio`, `crates/code`,
`crates/terminal`.

Obligations: a small hermetic test for new behavior; Tauri commands
for Aster desktop tenants; membership in the root Cargo workspace;
`publish = false`.

### Control-plane regime

Paths: `apps/cloud`, `apps/api`, `apps/auth`, `apps/billing`,
`apps/config`.

Obligations: bounded contexts (`domain/`, `application/`,
`infrastructure/`, `presentation/`, `published/`). Cloud schema
changes stay contiguous and bump `CLOUD_MIGRATION_COUNT` together
with `LATEST_CLOUD_MIGRATION_VERSION`. HTTP surfaces use
`#[controller]` with a guard and `auth.scopes`. A repository proof
is a dual-adapter proof: the same scenario runs on the in-memory
adapter and on live PostgreSQL. PostgreSQL harnesses use
`A3S_CLOUD_TEST_POSTGRES_URL` and `--test-threads=1`.

`apps/api` and `apps/cloud` keep their own Cargo workspaces. They
are not members of the root workspace.

### Shared-crate copies

Paths: `crates/acl`, `crates/boot`, `crates/orm`, `crates/event`,
`crates/lane`, `crates/flow`, `crates/sms`, and the same package
names under `apps/api/crates` and `apps/cloud/crates`.

A one-sided edit that grows the drift set fails
`tests/crate_drift_report.py`. Copies stay separate until a named
reconcile. A package is not moved across workspaces.

## Core Principles

### I. Fail Closed

Every refused condition names its reason. Denied and missing share
the not-found contract where the caller must not learn which one
occurred. An unset required value refuses activation instead of
defaulting. A misconfigured optional surface degrades to disabled
with an error-level log. It never takes down a process that can
still serve. **NON-NEGOTIABLE.**

### II. Evidence Retained

A landing that changes behavior records
`docs/evidence/<domain>/<date-slug>/OUTCOME.md` with the commands,
the counts, and an Explicitly not claimed section. Evidence
directories are not deleted, and a claim is not made without the
record. **NON-NEGOTIABLE.**

### III. Tests Follow the Path Regime

On a product path, new behavior has a small hermetic test in the
owning package. Ordinary desktop or CLI work does not require a
live PostgreSQL campaign.

On a control-plane path, a repository or authority proof follows
the control-plane regime: the same scenario runs on both adapters.
After a mass-failure run, re-run each distinct failure alone before
believing it. **NON-NEGOTIABLE.**

### IV. Closed Sets

Enums with `parse()` / `as_str()` pairs. Parse errors name the
refused set. Unknown values are refused at the boundary, never
coerced. **NON-NEGOTIABLE.**

### V. Workspaces and Published APIs

Library code does not depend on a desktop shell. On a product path,
Aster desktop tenants share the root Cargo workspace and lockfile
and do not grow another workspace. Cross-crate use goes through the
owning crate's public API. Preserve upstream package names and
public Rust APIs unless a task explicitly requires a rename.

On a control-plane path, `apps/api` and `apps/cloud` keep the
workspaces named in the control-plane regime. Domain code does not
import presentation or infrastructure. Cross-context data crosses
`published/` types.

### VI. Schema Changes Follow the Path

A schema change stays in the crate or app that owns the store.
Values are bound parameters. A raw query needs a documented reason
that the typed builder cannot express it.

On a product path the change stays in that owner. On a
control-plane path the migration identity follows the control-plane
regime. Do not add a second root Cargo lockfile.

### VII. Shrink-Only Ratchets

An existing allowlist (native dropdowns, facade debt, or any other
ratchet in this tree) may only shrink. Fix the underlying
condition. Never edit the allowlist to pass. **NON-NEGOTIABLE.**

### VIII. Replaceable Transports

A library that offers more than one transport keeps the choice out
of consumers. Transport fields do not leak into a shared envelope.
Product configuration of a transport is an ACL block.

### IX. One Configuration Language

ACL (`.acl`), parsed only by `a3s-acl`, is the only product
configuration language. Never create, restore, document, or parse
another product configuration format. **NON-NEGOTIABLE.**

Feature-consumed definition files are data, not configuration, on
the same terms as `SKILL.md`: closed-set fields, content shared
like source, and authority that the loading feature only narrows.
A definition must not name a provider, alter permission policy, or
join layered configuration discovery. Any new surface that answers
"how does the product behave" is configuration and goes in ACL.

### X. Commands, Controllers, and Test Files

On a product path, a Tauri command is registered and allowed by a
permission on the calling window's capability. Applet windows do
not receive host permissions they do not need.

On a control-plane path, an HTTP surface follows the control-plane
regime for `#[controller]`, the guard, and `auth.scopes`.

Tests live in sibling files behind `#[cfg(test)]`. Do not append
thousands of inline test lines to a production file.

### XI. Independent Judgment

Disagree when the evidence supports disagreement, and say so with
the reasoning. Do not implement a request that the evidence
contradicts without first stating the contradiction and the
corrected approach. Changing position requires new evidence, not
pressure. **NON-NEGOTIABLE.**

### XII. First-Principles Understanding and Adversarial Review

Before implementing, understand why the user needs the change.
A fix that leaves the cause in place is not done. After
implementing, try to break the change and look for what the tests
miss. A disproven finding stays recorded. A confirmed finding that
is not fixed names the exact follow-up. **NON-NEGOTIABLE.**

### XIII. One Repository

New Aster code lands in this repository. Do not create a Git
repository, a GitHub project, or a sibling checkout beside `aster`
for a library, product, or tool. Rust libraries go under `crates/`,
applications under `apps/`, and TypeScript or React libraries under
`packages/`. A second checkout of this repository is a git worktree.
The user-authorized desktop source boundary is the private submodule at
`apps/studio` (`git@github.com:kense-labs/studio.git`, `main`). It remains in
the root Rust workspace and uses the root lockfile; the parent tracks only
its gitlink, never desktop source files.
**NON-NEGOTIABLE.**

### XIV. Names Are Not Guessed

A name, entity, or alias comes from a model-supplied entity list,
a quoted span the author wrote, or an exact string already stored.
Do not infer names from capitalization, character classes, verb or
stop lists, or a regular expression tuned to an example sentence.
A missed example is not a reason to grow a matcher. Ranking tokens,
secret redaction by a known prefix, and a closed product word list
are not name extraction. Those lists do not grow to cover one
missed sentence. **NON-NEGOTIABLE.**

### XV. Meaning Is Not Matched

A decision about what the user means is made by the model or by a
typed closed set the host declared. A regex, substring, verb list,
or character class must not make that decision. Pattern matching
may only narrow fail-closed, rank candidates, or parse a mechanical
form. A missed phrasing uses the model path. **NON-NEGOTIABLE.**

### XVI. Pickers Are Drawn

No product surface ships an operating-system dropdown: no native
`<select>`, `<datalist>`, or a script-built `select` element. A
choice list is the picker in `DESIGN.md` §6. The Radix `Select`
list is shrink-only. A test fixture that exists to exercise a
native control is not a product surface.

### XVII. Dialogs Stay in Place

A modal dialog, including a desktop side panel with `role="dialog"`,
fades and scales in place. It does not slide in from outside the
window, and its motion does not animate `transform` or `filter`.
An anchored menu may rise at most 0.5rem. A phone-width bottom
sheet is the only edge travel. The contract is `DESIGN.md` §6
"Dialogs" and development-standards §16.

## Constraints

- Rust toolchain `1.97.1`. The root workspace, `apps/api`, and
  `apps/cloud` each keep their own lockfile. `publish = false`.
- Internal `a3s-*` crates resolve to a path in this repository.
- Do not restore an upstream binary update that replaces Aster with
  an A3S or Grok release.
- Production paths carry no `unwrap()` or `expect()` without a
  construction-level infallibility argument in a comment.
- Bearer comparisons use constant-time equality.
- Never commit another session's in-flight edits. Stage files
  individually and verify `git diff --cached --name-only` first.
  Do not commit unless the user asked.
- `.gitattributes` owns cross-platform line endings: UTF-8/LF source text,
  CRLF Windows command-file checkouts, and exact-byte `-text` evidence and
  fixtures. Investigate whole-file staging churn before committing; never
  sweep unrelated edits into renormalization. Development standards §10
  defines the compatibility checks and explicit encoding exceptions.

## Development Workflow

`AGENTS.md` carries the actionable Rust and cross-platform implementation and
review rules; development standards §17 explains the Microsoft adaptation.
Apply them to new and materially changed handwritten code within
the owning path's regime. Record confirmed legacy findings and coverage limits;
do not infer whole-tree compliance from searches or lint success. This workflow
does not authorize a public API, workspace, allocator, or toolchain migration.
Preserve supported target baselines and record native verification for affected
platforms; cross-compilation does not establish runtime compatibility.

A behavior change lands with its hermetic test and, when recorded,
`docs/evidence/<domain>/<date-slug>/OUTCOME.md`. A structural
migration lands as one commit. Bounded epics may use the spec-kit
phases under `specs/<NNN-slug>/`. The implementation evidence still
lands in `docs/evidence/`. Planning and acceptance are not two
copies of the same record.

## Governance

The constitution is amended through the same evidence discipline.
The behavior change lands first. The principle text follows.
