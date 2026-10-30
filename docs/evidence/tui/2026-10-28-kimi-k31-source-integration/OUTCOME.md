# Source-built Aster TUI with Kimi K3.1 Pro

Date: 2026-10-28. Platform: native Windows x86_64 / ConPTY.

The subsequent native Linux/WSL qualification is recorded separately in
[its own outcome](../2026-10-28-wsl-linux-k31-source-integration/OUTCOME.md).

## Result and source provenance

Aster 0.1.4 and local Code Core/ACP 9.1.6 build together and communicate through
the automatically discovered sibling ACP executable. The public Code `main`
checked during this work was [`cfaa22e8f1fca7ad1bfe132ac97ce7be930071f2`](https://github.com/A3S-Lab/Code/commit/cfaa22e8f1fca7ad1bfe132ac97ce7be930071f2)
(9.1.4). The newer maintained local sources were retained. No remote binary
replaced Aster. [build.json](build.json) records the source hashes, executable
hashes, toolchain, HEAD and matching pair identities.

The final continuous run passes: **10 passed, 0 failed, 0 ignored**, in
**557.98 seconds**. All 12 case artifacts use the same final source-pair
identity, finish their assertions, preserve the ACL and report no panic.
There are 118 successful step oracles. See
[tui-final-all-ten.log](tui-final-all-ten.log) and
[qualified-summary.json](qualified-summary.json). Qualified focused reruns
are also retained separately.

Every inference uses the original `./.a3s/config.acl`, parsed by `a3s-acl`, with
`kimi/k3.1-pro`. Each case checks that the ACL bytes are unchanged. The config,
keys and account state are not copied into the evidence. Each child has an
isolated home and workspace and disabled ambient account imports. The model
selector temporarily selects an existing Gateway catalog entry without sending
inference to it, then restores Kimi before any further inference.

## What changed

- Windows file observations now use anchored directory/file handles, bounded
  byte hashing and before/after identity checks. Directory reparse points,
  path escapes and alternate streams are refused. An unavailable root remains
  unobservable rather than being reported as a missing file. This replaces
  non-Unix stubs which prevented verified Windows mutations.
- The existing verification grammar recognizes the narrow PowerShell form
  `Get-Item -LiteralPath PATH -ErrorAction Stop`. A successful command must
  still bind the observed post-change bytes. Bare `Test-Path`, interpolation
  and extra command syntax do not qualify.
- The repository tool contract explains that line-numbered read text is a
  display representation. Exact copies must preserve raw content, encoding
  and final-newline state.
- A Rust live-provider PTY suite drives the real shipped entry point, sibling
  ACP, native permission/form UI and persisted session store. Completion is
  bound to the parent session, not child-session events. The driver focuses
  the composer and uses bracketed paste before submitting literal commands.

## Reachability and usability coverage

The suite covers the 28 commands actually offered by the ordinary Aster
palette plus the mode-specific `/expand` and `/fullscreen` commands. It does
not infer availability from names: palette rows and actual effects are both
checked. The surface case includes 60x20, 80x24 and 120x40 PTY resizes.

| Scenario | Observable acceptance | Evidence case |
| --- | --- | --- |
| Complete offered command tour | Local commands without model calls; real export/copy bytes; context/search/jump; persisted appearance/Vim settings; model and effort selection; multiline, mouse reporting state; external editor process; compaction fact; continue/resume identities; new/home/delete; renderer changes; clean exit | `surface.json` |
| Unseen file read | Actual read invocation and random file token in the settled Kimi reply | `read.json` |
| Allow-once mutation | Real permission cards; read/write/shell invocations; unchanged source and exact output bytes; verified terminal | `allow-once.json` |
| YOLO mutation | Same strict byte and verification checks with permissive session posture | `write-verify.json` |
| Reject and cancel permissions | Real permission cards; no target file before or after refusal/cancellation | `permission-reject.json`, `permission-cancel.json` |
| Interrupt running tool | Ctrl+C after an admitted sleeping shell call; idle UI; no delayed write after eight seconds | `interrupt.json` |
| Durable goal controls | Actual created/paused/resumed/cancelled facts | `goal.json` |
| Native form | Required and range validation; actual submitted name/number/boolean receipt; separate cancelled receipt | `form-submit.json`, `form-cancel.json` |
| Folded tool output | Previously hidden random tail of actual read output becomes visible with `/expand` | `expand.json` |
| Two-turn coding task | Read-only diagnosis; real edit; immutable checker; verified terminal; independently executed PowerShell checker | `coding.json` |

JSON evidence includes per-step terminal screens, source-pair identity,
committed tool/terminal facts and an ACL-unchanged result. Twelve cases map to
ten tests because permission and form tests each exercise two isolated cases.
`*-focused-pass.json` retains the qualified focused results before the final
continuous run. Earlier failed attempts are retained alongside them.

## Build and checks

Commands run from the repository root with the pinned Rust 1.97.1 toolchain;
the Studio submodule was initialized before Cargo operations.

```powershell
cargo build --locked -p aster -p a3s-code-acp
cargo test --locked -p a3s-code-core --lib -- porcelain::windows verification::tests:: --test-threads=1
cargo test --locked -p a3s-code-acp --lib -- output_check form_request session_setup reasoning_transport --test-threads=1
cargo fmt -p aster -p a3s-code-acp -p a3s-code-core -- --check
```

The paired build passes. Core: **62 passed** (including four real Windows
filesystem tests and the PowerShell command grammar). ACP: **31 passed**.
Focused formatting passes. The logs are retained in this directory.

The existing workspace guards were also executed under WSL Python:
`tests/workspace_dependencies.py`: **6 passed**; `tests/cli_pair_delivery.py`:
**10 passed**. For the shell delivery fixtures, only the two shell scripts'
checkout line endings were temporarily made LF and then restored; neither
script has a task diff. The workspace dependency result was observed in the
tool output; a separate raw log was not retained. Delivery output is retained
in `tui-cli-pair-delivery.log`.

Reproduce all live scenarios after rebuilding the pair:

```powershell
$env:A3S_CONFIG_FILE = (Resolve-Path .a3s/config.acl).Path
$env:A3S_TUI_LIVE_EVIDENCE_DIR = Join-Path (Get-Location) 'docs/evidence/tui/2026-10-28-kimi-k31-source-integration'
cargo test --locked -p a3s-code-acp --test aster_tui_live_e2e -- --ignored --test-threads=1 --nocapture
```

The tests are explicitly ignored by ordinary hermetic test runs because they
require the selected live provider. They were explicitly executed here; they
are not counted as passing skips. Tests themselves are in the pair source
fingerprint, so rebuild **both** executables after editing them.

The final continuous run invoked the already compiled test executable from
the preceding Cargo test build directly, with the same two environment
variables:

```powershell
& ./target/debug/deps/aster_tui_live_e2e-931eb4a332f55eae.exe --ignored --test-threads=1 --nocapture
```

## Counterexamples and limitations

The first exact-copy run introduced a final newline; strict byte equality
rejected it. The product tool-contract guidance was corrected and the prompt,
fixture shape and strict byte grader were retained. Another mutation run could
not establish Windows file effects before the handle-based implementation.

Driver/expectation defects were separated from product failures: child facts
could falsely terminate a parent turn; numeric JSON `4.0` is the form's valid
numeric value; ConPTY consumes mouse mode escape sequences; Windows uses an
ASCII composer marker; deleting a session removes its persisted summary and
then may prebind a new welcome session. Those assertions now check the actual
contracts. Literal command input also needed composer focus; prior fast input
could be interpreted as scrollback navigation. Original failed evidence is
retained, not presented as successful acceptance.

One malformed export input in an earlier driver attempt wrote a synthetic
transcript to `C:\Windows Temp\aster-kimi-KRNFND\workspace\exported.md`.
Automatic approval review rejected the attempted removal with **blocked by
policy**. The removal was not retried through another tool; that temporary
file remains. This file is a test transcript, not an ACL or account export.

Required `cargo fmt --all -- --check` is **not green**: native Windows hits
OS error 206 (command line too long); the WSL invocation runs and finds existing
format differences in Gateway, OCR-model, Power and the Studio submodule.
See `tui-whole-workspace-fmt-final.log`. Those unrelated worktree edits were
preserved. A speculative WSL source build was stopped before completion and
is not qualification evidence.

## Explicitly not claimed

- A complete workspace/CI pass, a release build, a commit or a push.
- Linux/macOS live E2E, screen-reader accessibility, or a human mouse audit.
- Every optional Core tool, third-party MCP service, remote sandbox, desktop
  tenant, account integration or the default planning-mode workflow. The
  live suite uses `--no-plan` and the offered ordinary Aster TUI surface.
- Arbitrary arithmetic correctness from the coding fixture: its frozen
  checker proves only its two stated input/output cases.
- That model/network nondeterminism cannot fail a future live run, or that
  unrelated pre-existing workspace changes were qualified by these tests.
