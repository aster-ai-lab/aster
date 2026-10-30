# Aster

Aster is one Rust workspace for the terminal coding tool, harness, and shared Aster desktop host.

## General rules

- Do not create additional Git worktrees without the user's explicit permission
  for the current task. Work in the current checkout by default.
- Do not add complexity for speculative races or security boundaries that are
  unlikely to occur. If one is genuinely required, first explain the concrete
  failure, its likelihood, and the code cost, then get approval. Never expand
  the change scope silently.
- Validate external input once at the entry boundary. Pass typed values inward;
  do not repeat defensive checks throughout internal code.
- Do not extract a single-use function prematurely. Extract only for reuse,
  genuinely complex logic, or a concept that benefits from a clear name.
- Keep the main function on the normal path. Put validation and details in
  small helper functions below it.
- Comment only on non-obvious constraints and counterintuitive behavior. Do not
  narrate code whose behavior is already clear.
- Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.
- Never hand-edit generated files. Change the source and regenerate them.
- Test the real implementation. Do not copy business logic into tests, and keep
  mocks to a minimum.
- Follow the repository's existing workflows. Do not introduce tools or
  processes that the repository does not already use.

## Task and artifact fidelity

- Treat the source data and delivery contract as authoritative. Preserve names,
  ordering, record counts, units, truncation, and required output paths. Do not
  normalize labels, add rows or summaries, or substitute an output location
  unless the task asks for it.
- When a task can be completed with reasonable assumptions, proceed and state
  them. Ask for clarification only when missing information materially changes
  the outcome; do not invent conflicts between compatible requirements.
- Re-open deliverables and compare them with the source before completion.
  Check record counts, categories, formulas, filenames, and required structure.
  Keyword presence alone is not acceptance evidence.
- Keep evaluation inputs, prompts, models, timeouts, and frozen graders fixed.
  Never hardcode answers or change a grader to obtain a pass. Use actual logs
  and artifacts to distinguish model errors, harness failures, and grader
  defects.

- Build from the repository root with the pinned Rust toolchain.
- `apps/aster` is the `aster` terminal binary. `crates/code/acp` is its
  `a3s-code-acp` companion. Build and distribute both together.
- macOS CLI releases include the complete verified sandbox companions.
  Packaging, installation and self-update must reject missing companions.
  Before creating or uploading a Release, install the final archive into an
  isolated prefix and pass `tests/cli_sandbox_smoke.py` using its installed
  CLI, ACP and sandbox. Reused build artifacts follow the same gate.
  Preserve missing-file regressions and actual command-execution assertions;
  startup, model acknowledgements and successful installation are insufficient.
- Apply the same completeness checks to other release prerequisites: native
  libraries, OCR models, fonts, Office/ACP companions, capture helpers and
  renderer entry assets. Required membership comes from the owning source
  contract, never only from the archive's own file list. Check the final
  desktop archive before generating or publishing its channel manifest.
- `crates/code/core` is the embeddable Rust SDK and harness. Do not add
  Node, Python, or Go bindings, SDKs, or package publishing jobs.
- `apps/studio` is the shared desktop host; `just build-studio` builds it with
  ACP and Office. Its default `aster` tenant is Aster. Keep tenant
  identities and data isolated. It is a private submodule from
  `git@github.com:kense-labs/studio.git`, tracked on `main`; never add its
  source files to the parent repository. Initialize it before Cargo commands.
- Office lives in `packages/office` and `crates/office`; it must build without
  the retired application. Its required sources have been merged.
  Do not import old user state or add a second Cargo workspace or lockfile.
  The `aster` build has no accounts; `toyou` retains managed authentication.
  See `specs/004-desktop-default-aster`.
- All members have `publish = false`; A3S dependencies, including optional
  ones, must resolve locally. `tests/workspace_dependencies.py` guards this.
- Keep internal A3S dependencies local. Preserve the upstream package names
  and public Rust APIs unless a task explicitly requires renaming them.
- Configuration stays ACL, parsed by `a3s-acl`. Preserve host/workspace
  trust boundaries, sandbox enforcement, bounded reads, and error handling.
- `crates/terminal` preserves the upstream pager layout and license notices.
- Use `cargo fmt --all -- --check`, focused tests, and the workspace checks
  in CI. Test new behavior with small hermetic cases.
- Do not restore upstream binary updates for Aster. Updates must not replace
  this product with an A3S/Grok release.
- Migration history and source provenance live in `docs/migration.md` and
  `docs/source-manifest.json`. Never copy credentials or user session state.
- The development standards are `docs/development-standards.md`. The
  enforceable distillation is `.specify/memory/constitution.md`. Bounded
  epics use `docs/spec-driven-development.md`. A NON-NEGOTIABLE principle
  cannot be waived by a spec, a plan, or a deadline.
- The constitution's Regimes table picks the rules for a path. Product
  paths (`apps/aster`, `apps/studio`, `crates/code`,
  `crates/terminal`) use hermetic tests and the root workspace.
  Control-plane paths (`apps/cloud`, `apps/api`, `apps/auth`,
  `apps/billing`, `apps/config`) use bounded contexts, contiguous cloud
  migrations, `#[controller]`, and dual-adapter proofs. Read that regime
  before editing `apps/cloud` or `apps/api`.
- Fail closed. Do not guess a name or a meaning with a regex or a word
  list. Do not ship a native `<select>` on a product surface. Do not
  create a repository outside this one.
