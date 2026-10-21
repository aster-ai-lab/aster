---
description: "Task list template for feature implementation"
---

# Tasks: [FEATURE NAME]

**Input**: Design documents from `specs/[###-feature-name]/`

**Prerequisites**: plan.md (required, including its Constitution Check),
spec.md (required)

**House rule**: every task that changes behavior carries its hermetic
test in the same task. A task without that test is not done.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (for example US1)
- Include exact file paths in descriptions

## Path Conventions

- Aster CLI and harness: `apps/aster/`, `crates/code/`
- AsterDream: `apps/work/`
- AsterDream: `apps/desktop/`
- Libraries: `crates/<name>/`
- Evidence: `docs/evidence/<domain>/<date-slug>/OUTCOME.md`

## Phase 1: Behavior and tests

- [ ] T001 [US1] [Closed set or refusal, with the hermetic test beside it]

## Phase 2: Wiring

- [ ] T002 [US1] [Callers and permissions. A Tauri command is allowed only
      on the capability that should call it.]

## Phase 3: Evidence

- [ ] T003 [US1] `OUTCOME.md` with Result, What landed, and Explicitly
      not claimed, when this landing is recorded
- [ ] T004 Stage files individually and verify
      `git diff --cached --name-only` before any commit the user asked for

## Final Verification Gate

- [ ] The owning package's focused test passes
- [ ] `cargo fmt --all -- --check` is clean for Rust edits
- [ ] No new crates.io `a3s-*` dependency
- [ ] No native `<select>` added on a product surface
- [ ] Product configuration, if added, is ACL
