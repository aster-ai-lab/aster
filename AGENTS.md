# Aster

Aster is one Rust workspace for the terminal coding tool, harness, and shared Aster desktop host.

## Execution and completion

- Define completion from the user's request and applicable repository contracts.
  Include necessary callers, tests, fixtures, configuration, and exports; do not
  invent additional acceptance goals. Execute small, clear tasks directly.
- Read applicable directory instructions and the owning path's regime before
  editing. Load reference documents when their subject affects the task; do not
  require a full repository map or all standards for a routine change.
- Use `rg` or `rg --files` to locate relevant code. Start with the entry point
  and direct dependencies. Expand a search to answer a concrete unresolved
  question that can change the implementation or verification.
- Reuse existing code and conventions. Implement the first approach that fully
  meets the constraints, then verify it. Add no optional features, dependencies,
  abstractions, or configuration for hypothetical future needs.
- For a bug, inspect callers and establish the failure with a focused test,
  reproduction, or log before fixing the shared cause. Use observed results to
  choose the next step; reopen a decision only when new evidence contradicts it.
- When a task can be completed with reasonable assumptions, proceed and state
  them. Ask for clarification only when missing information materially changes
  the outcome; do not invent conflicts between compatible requirements.
- Record unrelated findings briefly without fixing them unless they block the
  task. Continue authorized edits and local checks without repeated approval,
  while preserving the explicit approval requirements below.
- Finish when the requested behavior, necessary related changes, required
  checks, and final diff review are complete. Report the result, actual
  verification, and remaining limits; do not start another improvement pass.
  If blocked, name the missing prerequisite without claiming completion.

## General rules

- Do not create additional Git worktrees without the user's explicit permission
  for the current task. Work in the current checkout by default.
- Inspect existing changes in the task's paths and preserve unrelated work.
  Do not commit unless the user asks; stage only this task's changes and inspect
  `git diff --cached --name-only` before committing.
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
- Fail closed. Do not guess a name or a meaning with a regex or a word
  list. Do not ship a native `<select>` on a product surface. Do not
  create a repository outside this one.

## Verification

- For Rust changes, build from the repository root with the pinned toolchain
  and the owning path's workspace. Initialize `apps/studio` before Cargo commands.
  Run `cargo fmt --all -- --check`, focused tests, and the workspace checks in CI.
  Test new behavior with small hermetic cases under the applicable path regime.
- For new or materially changed handwritten Rust, run
  `cargo clippy --locked -p <package> --all-targets --no-deps` with the owning
  package's supported features. Read candidate matches in context. Record
  confirmed findings, exact follow-ups, existing failures, and coverage limits;
  lint success is not full compliance.
- For documentation-only changes, check the affected references and
  `git diff --check`. Run additional checks when executable examples, build
  instructions, or affected tooling require them.
- Fix failures caused by the change. Identify pre-existing or environmental
  failures separately; never weaken a test or gate to obtain a pass. After
  required checks pass, repeat or broaden verification only for new changes,
  new failures, or a concrete unresolved risk.
- Behavior changes retain commands, counts, and an Explicitly not claimed
  section in `docs/evidence/<domain>/<date-slug>/OUTCOME.md` as required by
  the constitution. Never claim a check passed without its actual result.

## Rust implementation and review

These are the actionable rules for new and materially changed handwritten Rust,
adapted from Microsoft's [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/).
`docs/development-standards.md` §17 retains the rationale and source mapping.

- Preserve the path regimes, published Rust APIs, and generated-source
  boundaries. A guideline does not authorize a workspace or crate migration.
- Use enums and invariant-bearing types for admitted values. Validate a
  newtype in its fallible constructor; keep invariant-bearing fields private.
  Builders validate cross-field constraints at `build()`.
- Return typed recoverable errors at library boundaries and preserve their
  sources. Application code adds context through the existing `anyhow` stack.
  Match error types or stable codes; never branch on rendered error text.
- Input, I/O, and cancellation failures are not panics. Production `unwrap()`
  or `expect()` requires a construction-level infallibility argument nearby.
- Prefer item-scoped `#[expect(lint, reason = "...")]` for an expected lint.
  Target-dependent allowances need a reason. Do not broaden workspace allows;
  check `[lints] workspace = true` before relying on workspace lint settings.
- Public types support useful `Debug`; credentials, header values, and sensitive
  payloads stay out of `Debug`, `Display`, and logs. Test redacted formatters
  with synthetic secrets, including containing configuration types.
- Use structured `tracing` fields for diagnostics. Reserve stdout for intended
  CLI or protocol output; logging must not corrupt the wire stream.
- Prefer safe Rust and existing platform wrappers. Every handwritten `unsafe`
  block or impl needs a nearby `SAFETY:` argument about its actual invariants.
  Unsafe APIs document caller obligations in `# Safety`. Do not evade lifetimes
  or `Send`/`Sync`; performance-motivated unsafe requires measurements.
- Bound input bytes and work at the entry boundary. Keep blocking I/O and long
  CPU work off async executor threads; do not hold a blocking lock across
  `.await`. Long async computations yield and preserve cancellation. A running
  `spawn_blocking` closure is not made abortable by dropping its join handle.
- Keep public re-exports explicit and document module purpose, observable
  effects, and applicable errors, panics, cancellation, and safety contracts.
  Unit tests live in sibling files; public API integration tests use `tests/`.
  Test-only bypasses are absent from normal builds.
- Measure hot paths before changing allocators, hashers, or ownership layouts.
  Prefer existing abstractions; preserve the portable release baseline.

## Cross-platform compatibility

- Keep changed behavior compatible with supported Windows, Linux, and macOS
  targets and affected architectures. Isolate OS-specific code, dependencies,
  and FFI behind the appropriate `cfg`; unsupported operations fail explicitly.
- Use `Path`/`PathBuf` and `OsStr`/`OsString` for filesystem and process values.
  Do not build paths by string concatenation, assume a separator, force paths
  to UTF-8, or use lossy/case-folded text for path identity or access decisions.
  File references must match the actual filename's case.
- Pass subprocess executables, arguments, working directories, and environment
  through process APIs. Invoke a shell only when its semantics are required;
  use that shell's quoting and executable conventions. Never assume Bash on
  Windows, PowerShell on Unix, or a developer-specific tool installation path.
- Reuse existing platform adapters for process-tree cancellation, terminal/PTY
  behavior, file locking, permissions, symlinks, and replacement operations.
  Verify affected native semantics rather than assuming Unix behavior on Windows.
- Specify text encodings and newline policy when reading or writing files and
  capturing text output. Source text is UTF-8 without BOM unless its contract
  says otherwise; do not rely on Windows' locale codec. Binary data stays bytes.
- Follow `.gitattributes`: source text is LF; Windows `.bat`/`.cmd` checkouts
  are CRLF; byte-identified evidence and fixtures remain exact. Inspect unexpected
  whole-file changes and renormalize only individually reviewed source paths.
- Preserve target CPU, ABI, minimum-OS, libc, and linker baselines. Do not add
  machine-local `target-cpu=native` or platform flags to shared build settings.
- Use portable temporary directories in hermetic tests. For path-handling
  changes, cover spaces and non-ASCII names; make platform-specific permission,
  executable-mode, and symlink expectations explicit.
- Use the existing CI target matrix and run native checks for affected targets
  when claiming compatibility. Cross-compilation proves compilation, not runtime
  behavior; a Windows executable launched through WSL is not a native Linux test.
  Record OS, architecture, toolchain, features, and untested targets. If a host
  limitation prevents a full check, report it alongside any focused checks.

## Task and artifact fidelity

- Treat the source data and delivery contract as authoritative. Preserve names,
  ordering, record counts, units, truncation, and required output paths. Do not
  normalize labels, add rows or summaries, or substitute an output location
  unless the task asks for it.
- Re-open deliverables and compare them with the source before completion.
  Check record counts, categories, formulas, filenames, and required structure.
  Keyword presence alone is not acceptance evidence.
- Keep evaluation inputs, prompts, models, timeouts, and frozen graders fixed.
  Never hardcode answers or change a grader to obtain a pass. Use actual logs
  and artifacts to distinguish model errors, harness failures, and grader
  defects.

## Workspace and product boundaries

- `apps/aster` is the `aster` terminal binary. `crates/code/acp` is its
  `aster-acp` companion. Build and distribute both together.
- `crates/code/core` is the embeddable Rust SDK and harness. Do not add
  Node, Python, or Go bindings, SDKs, or package publishing jobs.
- `apps/studio` is the shared desktop host; `just build-studio` builds it with
  ACP and Office. Its default `aster` tenant is Aster. Keep tenant
  identities and data isolated. It is a private submodule from
  `git@github.com:aster-ai-lab/studio.git`, tracked on `main`; never add its
  source files to the parent repository.
- Office lives in `packages/office` and `crates/office`; it must build without
  the retired application. Its required sources have been merged.
  Do not import old user state or add a second Cargo workspace or lockfile.
  The `aster` build has no accounts; `toyou` retains managed authentication.
  See `specs/004-desktop-default-aster`.
- All members have `publish = false`; Aster dependencies, including optional
  ones, must resolve locally. `tests/workspace_dependencies.py` guards this.
  Preserve the upstream package names and public Rust APIs unless a task
  explicitly requires renaming them.
- Configuration stays ACL, parsed by `aster-acl`. Preserve host/workspace
  trust boundaries, sandbox enforcement, bounded reads, and error handling.
- `crates/terminal` preserves the upstream pager layout and license notices.
- The constitution's Regimes table picks the rules for a path. Product
  paths (`apps/aster`, `apps/studio`, `crates/code`,
  `crates/terminal`) use hermetic tests and the root workspace.
  Control-plane paths (`apps/cloud`, `apps/api`, `apps/control-plane`)
  use bounded contexts, contiguous cloud migrations, `#[controller]`,
  and dual-adapter proofs. Edge services live under
  `apps/control-plane` with their own Cargo workspace; do not add them
  to the root members. Read that regime before editing `apps/cloud`,
  `apps/api`, or `apps/control-plane`.

## Release and distribution

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
- Do not restore upstream binary updates for Aster. Updates must not replace
  this product with an Aster/Grok release.
- Follow `docs/release-governance.md` for product tag namespaces, Latest
  ownership, rolling channels, and published-release compatibility.

## Reference routing

- The development standards are `docs/development-standards.md`; read the
  sections relevant to the change. The enforceable distillation is
  `.specify/memory/constitution.md`. A NON-NEGOTIABLE principle cannot be
  waived by a spec, a plan, or a deadline.
- Bounded epics use `docs/spec-driven-development.md`. Routine fixes do not
  need a new spec directory or planning artifacts.
- Migration history and source provenance live in `docs/migration.md` and
  `docs/source-manifest.json`. Consult them for migration and provenance work.
  Never copy credentials or user session state.
