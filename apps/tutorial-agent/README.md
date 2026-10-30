# tutorial-agent

A small, local Rust host for the Aster documentation tutorial. It is a root-workspace member, uses the root lockfile, and does not ship a separate SDK or update channel.

Run commands from the repository root with the pinned toolchain and an initialized Studio submodule.

```sh
cargo check --locked -p tutorial-agent --all-targets
cargo test --locked -p tutorial-agent
cargo run --locked -p tutorial-agent --example offline_tui
```

After building the offline example, the native terminal regression can be run with:

```sh
a3s-test run apps/tutorial-agent/tests/tui.acl --tui-executable "$PWD/target/debug/examples/offline_tui" --tui-working-directory "$PWD" --tui-columns 120 --tui-rows 36 --json
```

The offline example uses fixed responses, a temporary workspace and an injected `LlmClient`. It reads its README, asks permission to call the harmless `project_label` tool, then asks a business question. No provider credentials or external network are needed. Enter any task, press F6 for the displayed confirmation, then F9 for the selected answer. The example is an interaction fixture, not evidence of real-model task quality.

To use a real provider, pass an existing trusted ACL and a disposable workspace:

```sh
cargo run --locked -p tutorial-agent -- /absolute/host.acl /absolute/practice-repo
```

To retain a session, explicitly supply a private directory outside the analyzed workspace and a host-selected ID:

```sh
cargo run --locked -p tutorial-agent -- /absolute/host.acl /absolute/practice-repo \
  --state /absolute/private-tutorial-state --session practice-1
cargo run --locked -p tutorial-agent -- /absolute/host.acl /absolute/practice-repo \
  --state /absolute/private-tutorial-state --session practice-1 --resume
```

An existing ID cannot be overwritten by starting a fresh session. Resume checks the saved workspace before opening it. State may contain conversation and tool data; it is not a public artifact.

| Key | Action |
| --- | --- |
| Enter | Submit a task while idle |
| Esc | Cancel the exact active Run; wait for settlement |
| F2 | Save an explicitly configured persistent session while idle |
| F3 / F4 | Scroll the interaction pane and read parameters |
| F5 / Alt+j | Select the next pending confirmation by exact ID |
| F6 / F7（或 Alt+y / Alt+n）| Approve / reject the displayed request |
| F8 / F9（或 Alt+o / Alt+a）| Select / submit a business answer |
| PageUp / PageDown | Scroll conversation output |
| Ctrl+C | Restore the terminal, cancel pending requests and close resources |

Requests exceeding the confirmation display bound cannot be approved. A question answer is not permission approval. Structured JSON forms are handled by the separate ACP example; this minimal TUI reports them as unsupported. Headless mode accepts `--headless 'prompt'`, enables only read/ls/project_label, and refuses interactive tools instead of waiting without a user interface.

The basic host explicitly disables automatic planning; the goal chapter enables it when binding a goal. A scripted chat response is not a valid plan-analysis response.

`src/chapters.rs` contains compiled examples for Harness composition, context, file memory and goal commands. These are explicit integration functions; starting the basic TUI does not automatically activate memory or create a goal. The website explains where to call them and which host services remain necessary.

The focused tests use temporary directories and fixed model responses for stream identity, selected approval, deny, invalid arguments, cancellation, persistence and plan revision conflicts. Platform sandboxes, real providers, MCP and remote memory require separate qualification.
