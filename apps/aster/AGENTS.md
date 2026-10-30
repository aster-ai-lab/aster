# Aster TUI

The `aster` terminal binary and its interactive and headless entry points. The
repository-root `AGENTS.md` also applies.

## Task and artifact fidelity

- Treat source data and the requested delivery contract as authoritative.
  Preserve names, ordering, record counts, units, truncation, and output paths.
  Do not normalize labels, add rows or summaries, or substitute an output
  location unless the task asks for it.
- When requirements are compatible, complete the task with reasonable
  assumptions instead of asking for clarification. Ask only when missing
  information materially changes the outcome; do not invent conflicts.
- Re-open generated artifacts and compare them with the source. Verify record
  counts, categories, formulas, filenames, and required structure. Keyword
  presence alone is not acceptance evidence. For record transformations, use
  `output_check`'s `record_provenance` action with source and output records,
  allowing only task-authorized derived keys.
- Keep evaluation prompts, models, timeouts, and frozen graders fixed. Never
  hardcode answers or change a grader to obtain a pass. Use logs and artifacts
  to distinguish model errors, harness failures, and grader defects.

## Build and verification

- Build from the repository root with the pinned Rust toolchain. Build and
  distribute `aster` and `crates/code/acp`'s `a3s-code-acp` companion together.
- Keep configuration in ACL parsed by `a3s-acl`. Preserve host/workspace trust
  boundaries, sandbox enforcement, bounded reads, and fail-closed behavior.
- Do not add Node, Python, or Go bindings, SDKs, or package publishing jobs to
  the embeddable Rust harness.
- Run `cargo fmt --all -- --check` and focused tests. Test real behavior with
  small hermetic cases; use real end-to-end tasks for delivery and output
  contracts. Do not copy business logic into tests or rely on mocks alone.
