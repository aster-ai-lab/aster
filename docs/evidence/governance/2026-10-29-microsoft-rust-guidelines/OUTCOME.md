# Microsoft Rust guidance adoption and conformance review

Date: 2026-10-29. Repository: `D:/code/aster`. Review base:
`3185a21747df621c569ad4828103b53e685a8203`; the working tree also contains
pre-existing changes from other work. This task changes guidance and records a
first conformance review; it does not commit or change production Rust.

## Result

Updated `AGENTS.md`, development standards §17, and the constitution's
Development Workflow. The guidance applies prospectively to new and materially
changed handwritten Rust. Existing failures remain visible rather than being
converted into a claim of compliance. No new NON-NEGOTIABLE principle, tool,
workspace, dependency, allocator, or lint-wide suppression was introduced.

The reference is Microsoft's [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/),
version 2026.6, consulted on the date above. The adopted topics and deviations
are linked in §17: lint overrides, redacted diagnostics, typed boundaries,
invariant-bearing types, public contracts, unsafe reasoning, and bounded async
work. Existing public error enums and the path regimes are preserved.

## Review scope

Manual review focused on provider/model/search configuration formatters, ACP
account import and logging, and representative startup and Win32 unsafe calls.
Clippy checked the default-feature Windows libraries of `a3s-code-core` and
`a3s-code-acp`. The terminal binary received a focused format check and selected
source inspection. Sources supporting the findings are identified by hashes in
`reviewed-files.json`; this is not an exhaustive file-coverage list.

## Confirmed findings and exact follow-ups

| ID | Priority | Location | Evidence and required follow-up |
| --- | --- | --- | --- |
| R1 | P2 | `crates/code/core/src/config/provider.rs:50`, `:104`; `config/search.rs:237` | `ModelConfig`, `ProviderConfig`, and `SearchEngineConfig` derive `Debug` over plain API-key strings; provider/model header values are also rendered. Formatting these values exposes the strings, including through containing configuration types. Implement redacted `Debug` while retaining the public field/API shape; test synthetic keys, header values, and nested configurations. No actual leaked key or external disclosure is claimed. |
| R2 | P2 | `crates/code/acp/src/codex_account.rs:83`; `grok_account.rs:68` | Successful opt-in account import emits a full email at INFO. The ACP binary enables its daily file layer by default with an INFO filter (`src/bin/a3s-code-acp.rs:279` and `:287`). Omit or redact the identity field and add a captured-event test showing a synthetic email is absent. Import opt-in is not a redaction mechanism. |
| R3 | P2 | `crates/code/acp/src/grok_account.rs:131`, `:141`, `:252`; `codex_account.rs:211` | External auth, catalog, or configuration files are read with unbounded `std::fs::read_to_string`. Set explicit per-file byte budgets and enforce them during the read before parsing, preserving each importer's optional/error contract. Test over-limit files through the real importer. The core's existing bounded-I/O implementation demonstrates the intended mechanism; its private functions are not directly callable from ACP. This finding does not depend on a file-replacement race. |
| R4 | P3 | `crates/code/core/src/tools/builtin/bash.rs:437`; `agent/completion_runtime.rs:97`; further locations in `lint-diagnostics.json` | The reviewed packages emit 18 `undocumented_unsafe_blocks` and 83 `allow_attributes_without_reason` diagnostics. Add actual safety arguments and justified, narrowly scoped lint expectations/allowances when changing the owning code. These counts are compiler diagnostics for this configuration, not 101 proven runtime bugs. |

### Candidates not reported as production defects

- `LlmConfig` already has a custom redacted formatter in `llm/factory.rs:58`
  and synthetic-secret assertions in `llm/tests.rs`. Its handling does not
  protect the separate configuration structs in R1.
- `porcelain/windows.rs:173` and `:186` already explain the live handle,
  writable storage, and successful initialization conditions for unsafe calls.
- `turn_cancel.rs` deliberately blocks a worker in a test; it is not a
  production async-blocking finding. The UID/GID unsafe calls inspected in
  `agent_api/session_sandbox.rs` were also in test code.
- A missing safety comment is not evidence that a particular unsafe operation
  is unsound. No new race or unsafe wrapper is proposed by this review.

## Verification

Commands ran from the repository root with Rust `1.97.1-x86_64-pc-windows-msvc`.
The available Python interpreter is recorded in the command JSON files. Python
was run with `-B`; UTF-8 mode was used when the Windows default GBK decoding
proved unsuitable for this UTF-8 repository.

| Check | Result | Record |
| --- | --- | --- |
| `rustup show active-toolchain` | Pinned 1.97.1 | `toolchain.log` |
| `cargo fmt --all -- --check` | Could not launch rustfmt: Windows OS error 206, command/file-name length | `format.log` |
| `cargo fmt -p aster -p a3s-code-core -p a3s-code-acp -- --check` | Reports existing formatting differences in `acp/src/agent.rs` and `core/src/porcelain/windows_tests.rs`; these files were not edited by this task | `focused-format.log` |
| `cargo clippy --locked -p a3s-code-core -p a3s-code-acp --lib --no-deps -- -W clippy::undocumented_unsafe_blocks -W clippy::allow_attributes_without_reason -W clippy::await_holding_lock -W unsafe_op_in_unsafe_fn` | Exit 0, core 102 warnings, ACP 20 warnings; 122 in reviewed packages, plus 5 dependency compiler warnings | `clippy.log`, `lint-diagnostics.json` |
| `python -B -X utf8 tests/workspace_dependencies.py` | 6 tests pass | `workspace-dependencies-utf8.log` |
| `python -B tests/governance_regime.py` | 3 of 4 tests pass; the negative obligation-mutation test fails | `governance-regime.log` |
| Isolated failed governance test and unchanged HEAD comparison | Same failure with original and revised standards | `governance-isolated.log`, `governance-baseline.json` |

The initial dependency check failed because a test used the Windows default
GBK codec to read a UTF-8 shell script. The isolated failing test and the full
six-test suite pass under `-X utf8`; the test source and assertions were not
changed. Both attempts remain recorded.

The pre-existing governance defect is in `tests/governance_regime.py:73`:
`section()` searches for any subsequent `###` before considering a nearer `##`.
For the standards' control-plane regime, it includes later sections whose
duplicate obligation tokens mask the test's removal. `governance-baseline.json`
shows all three token mutations are accepted on both HEAD and the working
tree for that document, while the constitution rejects them. Follow up by
correcting section boundary parsing and proving the existing negative cases;
do not change the expected assertions to obtain a pass.

## Explicitly not claimed

- Whole-repository compliance, all targets/features, Linux or macOS runtime
  qualification, or a complete unsafe, concurrency, dependency, or security audit.
- Runtime reproduction of credential disclosure, memory exhaustion, or UB.
  R1-R3 are source-confirmed contract gaps with their triggering conditions.
- Production fixes, a clean pre-existing working tree, successful full-workspace
  formatting, a passing governance suite, or a commit.
- That lint success means the 122 warnings are resolved, or that absence of a
  configured lint warning proves cancellation and lock correctness.
