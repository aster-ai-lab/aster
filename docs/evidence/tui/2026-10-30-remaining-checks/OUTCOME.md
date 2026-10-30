# Follow-up: Windows check blockers

This record follows the [Git query fix](../2026-10-30-git-read-query-stall/OUTCOME.md)
and [Codex discovery fix](../2026-10-30-codex-home-resolution/OUTCOME.md).
Status: the identified code/test blockers are repaired and the checks below
passed. Native Windows formatter invocation and Python host limits are
described explicitly below.

## Repairs

- The dirty-worktree cleanup test now compares the diagnostic against the
  provider's listed path, after checking that its canonical path matches the
  created worktree. It still requires cleanup to fail, the directory to survive,
  and the original uncommitted bytes to survive. Git test repositories now
  include spaces and Chinese characters in their paths.
- The retrieval benchmark captures the copyable rerank mode before moving its
  options into the builder. Its workload, warmup/measurement counts, modes,
  assertions, and acceptance thresholds are unchanged.
- The two Windows soak tests use the existing `windows-sys` dependency for
  process liveness and handle counts. Their cycle counts and leak limits are
  unchanged; unsafe calls now document their pointer/handle invariants.
- The HTTP redirect fixture reads bounded, complete request headers and checks
  EOF, instead of ignoring a possibly partial read. Its deadlines and redirect
  expectations are unchanged.
- The dependency guard explicitly uses UTF-8 for repository text and captured
  text output, and LF for its temporary Rust stub. This fixes the real Windows
  GBK decode error without changing any guard assertion.
- Restored the original missing `packages/intelligent-ui/fixtures/form.json`
  and `display.json`, byte for byte from commit
  `a0e64b702bac3095fece03b5be723b432632eb3d`. Existing Core/Form Core tests
  referenced these missing files. No test input was invented or grader changed.
  Their Git blob identities are respectively
  `cca5347bcc168892fd574b098b317f601c2158a2` and
  `f54fab94f6174ed81b6c94328cc793b07a53fc39`.
- Running the actual Form Core module then exposed a separate contract defect:
  **3 passed, 1 failed** because `catalog.generated.json` contained OpenUI's
  positional argument contract instead of the native JSON state-binding
  contract its generator/test specifies. Preserved all 12 OpenUI entries in
  the existing `catalog.ts` API and regenerated the 13-entry native JSON file
  using the existing `a3s-form-core json-render-catalog` command. The unchanged
  native test now passes. No generated source was hand edited, and no parser
  assertion or accepted input was changed. The package README documents the
  distinct source contracts and existing generator.
- Applied only the reported rustfmt differences in ACP `agent.rs`, Core
  `porcelain/windows_tests.rs`, Gateway's three entrypoint files, OCR Model's
  import entrypoint, Power's `auth.rs` / `unlimited_ocr.rs`, and eight Studio
  Rust files. The generated inline OCR payload was compared and preserved.
  Studio and Power edits stay inside their submodules; no submodule commit or
  gitlink update was made for this repair. Other existing edits were preserved.
  Gateway `inference/usage_report.rs` was also individually reviewed and changed
  only from CRLF to LF to clear its whitespace check; its source text is intact.

## Environment and verification

Native Windows x64, Windows build 22631, pinned Rust
`1.97.1-x86_64-pc-windows-msvc`, default Cargo features. Cargo ran from the root
workspace with Studio initialized. No Linux/macOS runtime result is claimed.

The host's default Python 3.13 installation cannot import `encodings`. The
already-installed Astral CPython 3.12.14 works. The dependency guard ran via
`py -V:Astral/CPython3.12.14 tests/workspace_dependencies.py`: **6 passed**.
For the existing MCP fixture launcher, the check process set
`PY_PYTHON3=Astral/CPython3.12.14`, so its existing `py -3` path selected that
working interpreter. No persistent Python setting was changed.

- `cargo clippy --locked -p a3s-code-core -p a3s-code-acp --all-targets --no-deps`:
  passed, with existing warnings. Lint success is not a full compliance claim.
- `cargo test --locked -p a3s-code-core --test test_mcp_stdio_soak -- --ignored --exact soak_mcp_stdio_add_call_remove_leaves_no_child --test-threads=1`:
  **1 passed**, exercising all **30** real MCP child add/call/remove cycles.

`cargo fmt --all -- --check` cannot launch rustfmt on this Windows host because
its expanded command line exceeds the native limit (OS error 206). The same
Cargo format check is therefore run separately for each root workspace member;
this changes neither the formatter nor its configuration.

## Final checks and paired build

The following focused checks passed on the native host:

| Check | Result |
| --- | --- |
| `cargo test --locked -p a3s-code-core --lib -- tools::builtin::git::tests llm::http::redirect_tests --test-threads=1` | 19 passed, including spaces/Chinese paths and refusal to remove dirty worktrees |
| `cargo test --locked -p a3s-code-core --test git_read_queries --test json_render_forms` | 1 Git batch + 2 original form integration tests passed |
| `cargo test --locked -p a3s-code-core --test test_session_open_close_soak -- --ignored --exact soak_two_hundred_session_open_close_cycles_stay_bounded --test-threads=1` | 1 passed, all 200 open/close cycles, original handle bound retained |
| `cargo test --locked -p a3s-code-acp --test codex_account_stdio` | 2 passed, 8 synthetic directory-discovery scenarios |
| `cargo test --locked -p a3s-code-acp --lib codex_account::tests` | 6 passed |
| `cargo test --locked -p a3s-form-core --lib json_render::tests` | 4 passed |
| `cargo clippy --locked -p a3s-form-core --all-targets --no-deps` | Passed |
| `cargo fmt -p <member> -- --check` for every root workspace member | 167/167 passed; checked unique member names and all recorded exits |
| `npm test -- --reporter=dot` in `packages/intelligent-ui` | 53 passed both before and after catalog repair |
| `npm run typecheck` in `packages/intelligent-ui` | Passed before and after catalog repair |
| `npm run build` in `packages/intelligent-ui` | Passed |
| `git diff --check` | Passed |

Together with the MCP soak above, these are **36 passed Rust test cases**,
**6 passed dependency guard cases**, and **53 passed frontend cases**. An initial
Form Core filter used `json_render_tests` and matched zero tests; that run is
not counted. It was corrected to the actual `json_render::tests` module, which
exposed and then verified the catalog repair.

Reopened both restored fixtures and compared their bytes to the historical
sources. Reopened the generated native catalog and compared it to the actual
generator stdout plus its final LF. Bundled the actual `catalog.ts` exports
with the package's existing esbuild and compared all 12 entries, their property
arrays, and component ordering to the original OpenUI source. All matched.
Root, Studio, and Power `git diff --check` checks also passed.

`cargo build --locked -p aster -p a3s-code-acp` passed after the final repairs.
Both reopened binaries reported the identical build identity:

```text
aster-cli-pair-v1:17a78bdbd33e51bc46be3833a236a60e9c9898df:x86_64-pc-windows-msvc
```

Versions: `aster 0.1.4 (b53a3cf9cdd9)` and ACP `9.1.6`. Terminal `--help` also
exited successfully. Final SHA-256:

```text
286628C8C621E73DD8DA37B461183C600143A5989F017B9B26F1A3F7D088C9DA  target/debug/aster.exe
B31EDBAE35CC18A186B6AF4910F91E8245B0902787E7CBF0E2A8BA80B306E58C  target/debug/a3s-code-acp.exe
```

These remain local source builds including other existing workspace edits.
Run `./target/debug/aster.exe` from the root to use this pair. The installed
official stable binaries were not overwritten. No commit or publication was
performed by this repair.

Existing nonblocking Clippy/compiler/linker warnings remain, including MSVC's
unused `ProjectedFSLib.dll` delay-load warning. The host's broken default
Python installation is not repaired; checks use its working installed 3.12
interpreter as described above. Full workspace tests, live model requests,
and native Linux/macOS execution were not run.
