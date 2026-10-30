# Aster TUI: matching Linux acceptance in WSL

Date: 2026-10-28. Host: Ubuntu 24.04.3 LTS, WSL2, x86_64 Linux.

## Result

Aster 0.1.4 and Code Core/ACP 9.1.6 build from this repository with the pinned
Rust 1.97.1 toolchain. Both executables are native Linux ELF binaries and have
the same `x86_64-unknown-linux-gnu` source-pair identity. Their output directory
is `target/wsl-linux`; the existing Windows executables are preserved.

The final continuous live run passes: **10 passed, 0 failed, 0 ignored** in
**575.29 seconds** (Rust test-harness time). All **12 cases** finish their
assertions without panic, use the same final source-pair identity, and preserve
the original ACL. There are **119 successful step oracles**, covering
**28 ordinary commands and two mode-specific commands**. See
[qualified-summary.json](qualified-summary.json) and
[tui-linux-final-live.log](tui-linux-final-live.log). Build identity, executable
hashes and source hashes are in [build.json](build.json).

## Same acceptance contract

The same ten Rust test entry points and twelve isolated cases as the
[Windows qualification](../2026-10-28-kimi-k31-source-integration/OUTCOME.md)
run through native Linux PTYs. Eight workflows reuse the existing scenarios.
Two formerly Windows-only entry points now also compile on Unix:

- Interruption uses `sleep 6; printf LATE > late.txt` in place of PowerShell.
  It still waits for an actual admitted shell invocation, sends Ctrl+C,
  requires the UI to settle and checks for no delayed write after eight seconds.
- The two-turn editing fixture uses `calc.sh` and `check.sh`. It retains the
  same two numeric input/output cases as the PowerShell fixture, a read-only
  diagnostic turn, a real edit receipt, an immutable checker, a bound verified
  terminal and independent host-side checker execution. `/bin/sh` runs the
  checker; `test -f calc.sh` supplies the literal existence check.

The existing Unix exact-copy prompt uses `test -f result.txt`. Strict byte
equality includes the absent final newline. Prompts, model, timeouts and
graders are frozen for the Linux run after these platform substitutions.

Coverage includes the 28 offered ordinary commands plus `/expand` and
`/fullscreen`; three terminal sizes; actual model reads and writes; allow-once,
YOLO, refusal and cancellation; durable goals; typed forms and validation;
folded tool output; settings and external editor; export and copy; compaction;
continue, explicit resume, new, home, permanent deletion and clean exit.
Unlike ConPTY, Linux PTY output also exposes the actual mouse mode escape
sequences, which the surface test checks.

Every inference uses the original `/mnt/d/code/aster/.a3s/config.acl` and
`kimi/k3.1-pro`, parsed through `a3s-acl`. The config is not copied or edited.
Each case isolates its home and workspace and checks the original ACL bytes
afterward. Provider secrets are redacted from retained terminal screens and
receipts. Model selection briefly changes a catalog entry without inference,
then restores Kimi before any further model request.

## Commands and supporting checks

From `/mnt/d/code/aster` inside WSL:

```sh
cargo build --locked --offline --target-dir target/wsl-linux -p aster -p a3s-code-acp
cargo test --locked --offline --target-dir target/wsl-linux -p a3s-code-acp --test aster_tui_live_e2e --no-run --message-format=json
```

The test executable is read from Cargo's `compiler-artifact` JSON record.
The final run invokes that exact ELF executable with
`--ignored --test-threads=1 --nocapture`, `A3S_CONFIG_FILE` naming the original
ACL and `A3S_TUI_LIVE_EVIDENCE_DIR` naming this directory. Each case checks the
actual TUI and automatically discovered sibling ACP have matching build IDs.
The exact executable command is retained in `qualified-summary.json`.

To reproduce the live suite using Cargo after building both binaries:

```sh
A3S_CONFIG_FILE="$PWD/.a3s/config.acl" \
A3S_TUI_LIVE_EVIDENCE_DIR="$PWD/docs/evidence/tui/2026-10-28-wsl-linux-k31-source-integration" \
cargo test --locked --offline --target-dir target/wsl-linux -p a3s-code-acp --test aster_tui_live_e2e -- --ignored --test-threads=1 --nocapture
```

The live tests are explicitly ignored in ordinary hermetic runs, and explicitly
executed for this acceptance. A skipped test does not qualify as passing.

Existing workspace guards run natively under WSL Python: **6 dependency tests
passed**, and **10 pair-delivery tests passed**. For the shell fixtures, the
two Windows-checkout scripts were temporarily normalized to LF, then restored
byte-for-byte; neither script has a task diff. Focused formatting passes:
`cargo fmt -p aster -p a3s-code-acp -p a3s-code-core -- --check`.

Linux focused Rust checks pass: **118 Core tests** (including Unix file
observations, bounded stamps, receipts and completion verification), plus
**31 ACP tests**. Commands:

```sh
cargo test --locked --offline --target-dir target/wsl-linux -p a3s-code-core --lib -- porcelain:: verification::tests:: --test-threads=1
cargo test --locked --offline --target-dir target/wsl-linux -p a3s-code-acp --lib -- output_check form_request session_setup reasoning_transport --test-threads=1
```

## Retained limitations and build attempts

The first continuous live run had **2 passed, 8 failed**. Its artifacts and
build identity are retained under `attempt1-input-timing/`. Terminal evidence
showed a stray focus key and Enter being incorporated into pasted text rather
than submitting commands. The driver now waits for a rendered, then cleared,
input marker, and waits for submitted text to render before sending Enter.
The existing exact parent-session prompt assertion ensures this marker never
reaches the model. Product code, frozen prompts and outcome graders were not
changed to obtain passing results. All **eight previously failing tests
passed when rerun individually**; `isolated-*.log` and
`tui-linux-isolated-rechecks.log` retain those results before the final suite.

The initial online Cargo attempt spent time refreshing incomplete WSL caches.
Offline attempts exposed missing/stale public registry entries. Existing
Windows Cargo registry index and archive caches were copied into the WSL
Cargo cache, then the paired locked offline build succeeded. No dependency
version or source was changed to bypass resolution. The initial attempts and
successful native build log are retained here.

`cargo fmt --all -- --check` is **not green**. The WSL invocation finds existing
format differences in Gateway, OCR-model, Power and the Studio submodule;
see `tui-linux-workspace-format.log`. These unrelated edits are preserved.

## Explicitly not claimed

- A full workspace/CI pass, a release build, a commit or a push.
- A rerun of the Windows acceptance after adding the Unix test adapters,
  a macOS qualification, or Linux distributions outside this WSL host.
- Screen-reader accessibility, a physical mouse audit, every optional SDK
  tool or external MCP service, remote sandbox or desktop/account integration.
- The default planning-mode workflow: the matching acceptance suite uses
  `--no-plan`.
- Arbitrary arithmetic correctness beyond the coding fixture's two frozen
  checker cases, or immunity to future model/network nondeterminism.
