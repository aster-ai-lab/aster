# Aster

Aster is one Rust workspace for the terminal coding tool, harness, and Work and AsterDream desktop hosts.

- Build from the repository root with the pinned Rust toolchain.
- `apps/aster` is the `aster` terminal binary. `crates/code/acp` is its
  `a3s-code-acp` companion. Build and distribute both together.
- `crates/code/core` is the embeddable Rust SDK and harness. Do not add
  Node, Python, or Go bindings, SDKs, or package publishing jobs.
- `apps/studio` is the shared desktop host; `just build-studio` builds it with
  ACP and Office. Its default `aster` tenant is AsterDream; explicit `toyou`
  retains toyou. Keep their product identities and data isolated.
- `apps/work` retains legacy source and the Office vendor dependency.
  `just build-work` remains a legacy build. Do not import old user state or
  add a second Cargo workspace or lockfile. See `specs/004-studio-default-aster`.
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
  paths (`apps/aster`, `apps/work`, `apps/studio`, `crates/code`,
  `crates/terminal`) use hermetic tests and the root workspace.
  Control-plane paths (`apps/cloud`, `apps/api`, `apps/auth`,
  `apps/billing`, `apps/config`) use bounded contexts, contiguous cloud
  migrations, `#[controller]`, and dual-adapter proofs. Read that regime
  before editing `apps/cloud` or `apps/api`.
- Fail closed. Do not guess a name or a meaning with a regex or a word
  list. Do not ship a native `<select>` on a product surface. Do not
  create a repository outside this one.
