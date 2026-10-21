# Native task archive regression

This suite connects the compiled production renderer to an isolated real Rust
agent plane and disk store. The server injects a private loopback runtime and
proxies requests; task list and mutation replies are never mocked. It seeds one
active task and one archived task. Use a fresh server for each run.

From `apps/desktop`, build the renderer and the native test executable:

```sh
DESKTOP_RENDERER_DIST=/tmp/a3s-desktop-functional-renderer pnpm exec rsbuild build
rustup run 1.97.1 cargo test --offline --manifest-path src-tauri/Cargo.toml --lib --no-run
```

Pass the executable printed by Cargo to the server in a separate terminal:

```sh
node tests/e2e/task-management/native-server.mjs <native-test-executable> /tmp/a3s-desktop-functional-renderer 4596
```

Then run the admitted suite with the installed compatible standalone
`agent-browser` (0.26.0, protocol 15 at verification):

```sh
a3s-test check tests/e2e/task-management/archive.acl --json
a3s-test run tests/e2e/task-management/archive.acl --browser-driver standalone --json
```

The suite checks the archive heading after selecting the ninth settings menu item.
That scoped selector avoids the background task's identically named archive
button; re-observe the settings navigation if its order changes. The confirmation
selector comes from the shared PopConfirm component. Neither selector executes
application code or substitutes task results.

Stop only this fixture with Ctrl+C. It closes its native child through stdin EOF
and removes the private runtime descriptor. The isolated state and sanitized
`requests.jsonl` remain in the printed temporary directory for inspection. Do not
retain the runtime descriptor, Authorization headers, or bootstrap script as
evidence. Reports and screenshots are under `.a3s-test/`.

This proves the browser/native-HTTP restore journey. Native Tauri window,
multi-WebView, real model, batch, query and complete R1–R3 acceptance remain
separate gates. The ignored Rust `serve_archive_e2e` test is a server entry point
explicitly executed by the fixture, not a skipped functional acceptance test.
