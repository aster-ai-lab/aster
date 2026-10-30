# Aster

Aster is one Rust workspace for the terminal coding tool, harness, and shared Aster desktop host.

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
