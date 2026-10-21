# Desktop workspace migration

Desktop is built from `apps/desktop` as `a3s-desktop`. Its native database runner
and lightweight desktop contract tests share the root Cargo workspace.

Validated locally on macOS with Rust 1.97.1:

- `just build-desktop`: Desktop, ACP, and Office executable builds pass.
- `cargo check --locked --offline --workspace --all-targets`: pass.
- `cargo fmt --all -- --check`: pass.
- `python3.13 tests/workspace_dependencies.py`: 4 pass, including hermetic
  companion staging and missing-release-companion refusal.
- Desktop native database and desktop contract tests: 14 pass.
- ORM engine and SQLite integration tests: 11 pass.
- Use Extension library tests: 190 pass, 1 upstream ignored.
- Desktop memory tests: 5 pass, including personal/private/room separation.
- Desktop launcher and dependency-lockstep tests: 10 pass.
- Work contribution-protocol tests after shared contract regeneration: 6 pass.
- Desktop frontend frozen offline installation and production renderer build: pass.

The source checkout's Desktop and two exclusive Intelligent UI frontend packages
are moved to Trash after validation. OS/backend services and their shared
frontend libraries remain under `../a3s`. Backend live tests retain their PDF
fixture under that checkout's `fixtures/paperloom` directory; its SHA-256 is
`1e2fd339bdc046bbcaac8d7c4777119bd7f1ac990ca5e378605796fb476fe081`.
Offline installation and Rust compilation are repeated after removing Desktop.

Full frontend type checking reports 324 diagnostics, compared with 349 in the
original Desktop checkout using the same command. Comparing file, error code,
and primary message yields no new distinct errors. CI gates Desktop's production
renderer and launcher tests; Work keeps its existing type-check gate.

No interactive GUI, signed release package, real model, cloud API, PostgreSQL
service, or hardware qualification is performed. Existing ignored tests stay
ignored. No credentials or user sessions are imported into Aster.
