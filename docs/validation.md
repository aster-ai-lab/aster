# Local validation

## Core／ACP 当前技术文档与发布证据

2026-11-05 更新的 [Core／ACP DeepWiki](architecture/aster-core-acp.md)
系统说明当前搜索预算、取消与截断语义、清单检索、增量主键、失败分类、
ACP 装配和发布门禁。相关结果为 278 项 Core／ACP 测试通过，Core／ACP
Clippy 通过，以及 13 成员 macOS 候选包的两种隔离安装执行验收通过。
这些结果不覆盖全部 TUI 改动，也不表示大型仓库默认索引预算必然达到 Ready。

详细证据：[Core／ACP 发布收敛](evidence/code/2026-11-05-core-acp-release-convergence/OUTCOME.md)。

## 历史验证

Validated on macOS / Apple Silicon with Rust 1.97.1 on 2026-10-18.
These results qualify the migration and local launch paths, not a release
or every optional upstream integration.

## Build and focused checks

| Check | Result |
| --- | --- |
| `cargo check --locked --workspace --all-targets` | Passed for all 135 members |
| Core `advanced-harness,deep-live` feature check | Passed |
| ACL library tests | 180 passed |
| Effect library tests | 72 passed, 1 existing ignored test |
| Memory library with SQLite | 74 passed |
| Aster entry point and workspace integration | 45 passed |
| Welcome layout, cached animation, clipping, and centering | 109 passed |
| ACP `v912_seam` integration | 7 passed, 1 retired oracle ignored |
| PTY startup, provider refusal, model switching, and mock chat | 6 passed, 2 macOS shell tests skipped |
| Core current memory-retention contracts | 14 passed |
| Core meta-harness composition | 19 passed |
| macOS native process-lifetime refusal | 1 passed; no process or file effects |
| Code and Work launchers | 3 passed: built executable, quoted arguments, failed builds, newest bundle, missing build |
| Complete internal dependency boundary | 2 passed; zero remote Aster packages in Cargo metadata/lockfile |
| Both Flow engines with all features | Passed with local Boot, Event, and ORM |
| Work native build (`just build-work`) | Passed; ACP and Office resources match the workspace binaries |
| Work platform and document-import regression | 32 passed |
| Use Extension library | 190 passed, 1 existing ignored test |
| OCR contracts without provider features | 4 passed |
| Fresh frontend install and postinstall | Passed with the frozen pnpm lockfile |
| Exported Git sources | Dependency guard and frozen frontend install/postinstall passed without local ignored files |
| Office editor and WASM kernel from source | Passed |
| Work production renderer | Passed |
| Work Web and aiCore typechecks | Passed |

The default product dependency tree resolves the imported Aster runtime
packages to this workspace. The source manifest's 135 package paths resolve
locally. Core's native sandbox and guest guards match the source checkout.
Code's Node, Python, and Go SDK packages are absent.

## Terminal evidence

Aster TUI welcome and slash regressions are driven by the paired Rust PTY /
ACP suites (`crates/code/acp/tests/shipped_tui_e2e.rs`,
`apps/aster/tests/tui_one_agent.rs`), not by an external A3S/`aster-test` TUI
driver. Historical welcome viewport text and VT recordings remain under
`evidence/terminal/` as retained artifacts only.

```sh
cargo build --locked -p aster -p aster-acp
cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1
python3 tests/just_work.py
```

After `just build`, `just code --version` runs the built CLI and reports its version.
`just work` was also used to launch the existing compiled Work bundle;
Work now builds from this same workspace; its frontend remains a local pnpm workspace.

## Known limits

Core's default Bash path uses ksandbox (Seatbelt on macOS). Out-of-workspace
writes stay blocked; process-host and yolo remain explicit opt-ins. MicroVM
isolation (kbox) is orthogonal and not required for Bash. File tools and
local mock-provider chat are exercised separately. See
[sandbox-control](architecture/sandbox-control.md) and
[Studio build prerequisites](../apps/studio/README.md).

The legacy ACP `v912_remember_before_stream_close` test expects raw user
input to be stored even when the model returns an empty extraction. Current
Core explicitly forbids that behavior and stores memory in Vec snapshots
rather than the oracle's JSON directory. This historical oracle is ignored;
the current memory-retention tests cover model authority, persistence, FIFO,
stream completion, and immediate session close.

The first expanded PTY run exposed these platform limitations and a model
picker test that submitted a follow-up before accepting reasoning effort.
The model picker test now accepts the effort menu and waits for the updated
composer status before sending the next turn.

Inherited warnings include floating-point fallback diagnostics in the
terminal web-search client and large macOS debug unwind sections. No real
provider credentials were used. Release builds, Linux/Windows execution,
external browsers and live optional cloud backends were not qualified in this local run.


## Work quality gates retained from the import

The full Work `pnpm typecheck` remains red with 17 errors in existing Node
and E2E files: the archived desktop build test, the composer-mode registry
shape, and E2E assertion/DOM types. Its Web and aiCore projects pass. The
production renderer and local Office declaration build also pass.

`pnpm test:lint` reports existing assertion, promise, and tsconfig errors.
`pnpm docs:check` reports historical links to missing legacy evidence and
retired sibling projects. These checks were run and were not weakened.
They are separate from the passing source/dependency, Rust compilation,
and frontend production-build checks above.

The migration restores executable bits on the positive extension fixtures,
keeps their resource requirements intact, and localizes the weather-schema
and ACP AIX fixtures. Work's integration-test module imports and hidden
platform exports now compile in the workspace. The document test fixture
uses atomically unique temporary directories so parallel tests cannot share
files accidentally.

CI compiles the combined Rust workspace, guards in-tree ownership and
`publish = false`, and installs/builds the Work frontend from a clean checkout.
Git preserves pnpm patch bytes so checkout normalization cannot invalidate
their locked hashes.
The Linux container package script and macOS release packaging paths were
updated; signed/release installers and non-macOS execution were not run here.

## macOS Guest command follow-up

The [2026-10-18 execution record](evidence/execution/2026-10-18-macos-guest-owner/OUTCOME.md)
retains two real owner-death failures before the fix and five passing native
cases on a frozen signed shim afterwards. The Aster consumer passed all seven
real Guest lifecycle checks, its 63-test sandbox family and the explicitly
invoked Bash routing soak. Core strict Clippy passed. Known execution refusal
is now visible in the first model request and disappears after recovery.

This is development evidence. The default macOS native guard stays refused;
Guest packaging, protected metadata policy, stale control-file recovery,
network mediation, background streaming and Python/openpyxl remain unqualified.
The fixture image and shim were supplied explicitly, so these results do not
qualify installation or an ordinary user's default command workflow.

## CLI P0, 2026-10-19

The [CLI P0 record](evidence/terminal/2026-10-19-cli-p0/OUTCOME.md) ran the
shipped debug `aster` binary on macOS: version, doctor, inspect, unconfigured
`setup --json`, source-build `update --check`, local session commands, one
loopback conversation with continue, resume, fork, and delete, a sandbox-profile
refusal, and the macOS shell refusal. 1 P0 process passed, 43 binary unit
tests passed, and 2 workspace tests passed. Developer `~/.aster` was unchanged.

This still does not qualify a signed installer, a release-profile binary,
Linux or Windows, or a live provider. The macOS native shell remains refused.
