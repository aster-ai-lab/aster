# Quark local browser search and account model

## Result

Implemented explicit, opt-in use of a logged-in local Quark browser:

- `import_accounts = ["quark"]` registers `quark/kbot` through the existing
  account import and model catalog. No provider protocol was added.
- `search.engine "quark"` plus `search.headless.backend = "quark"` selects the
  browser search source. `cdp_url` defaults to `http://127.0.0.1:9222`.
- The Browser crate attaches only to a validated loopback CDP origin and its
  loopback WebSocket. It does not read/copy cookies or close the user's browser.
- Kbot model calls use a separate Aster-owned window and fresh conversation.
  Search uses the existing page's CDP background resource loading, never starts
  Quark or creates/navigates/closes a page. Text prompts, responses, tools,
  cancellation and output reads are bounded; account tools remain governed by Aster.
- Terminal model selection starts the existing ACP sidecar, which performs the
  same trusted account import. The pager only stores the account opt-in.

Usage and limitations are documented in
`crates/code/manual/USER_GUIDE.md` and both Search READMEs.

## Default search preference follow-up

When neither the request nor ACL selects an engine, a successful local login
probe now puts `quark` before the existing API and HTTP/RSS fallbacks. Explicit
engine selections and an explicitly selected non-Quark backend are unchanged. A
failed probe retains the ordinary default cascade. This does not require the
model account import.

The default plan still satisfies the existing structural quorum before stopping.
Successful single-result live cases stopped after Quark; they did not bypass
multi-source requirements for larger requests. A separate live run recorded
Quark's typed `challenge`, then AnySearch results: Quark was still the first
executed engine, and the normal fallback remained intact.

After replacing the create-plus-goto double navigation with one post-create
navigation:

- the direct logged-in probe passed 10/10 consecutive runs;
- Core `web_search` with no `engines` passed 3/3 consecutive live runs, reported
  `engine_selection_source = "logged_in_quark"`, and executed only `quark` in the
  single-result case;
- the CLI with no `--engines` and no explicit browser selected Quark first,
  returned a Quark result and reported zero failures;
- the Search library passed 561/561 unit tests, including the default-order and
  explicit-selection regressions;
- Core `web_search` passed 59 tests with 4 existing external/soak tests ignored;
  the unavailable-login probe and explicit non-Quark backend tests passed;
- Browser, Search and Core all-target/no-deps Clippy commands exited 0.

Earlier probes that fell back to AnySearch/Tavily were not counted as Quark
successes. The final repeated runs above used the single-navigation
implementation.

## Environment and live evidence

Tested on macOS aarch64 with Quark 7.5.4.1068 (Chrome/144.0.7559.86, CDP 1.3).
The user explicitly authorized restarting the installed browser with:

```bash
open -a /Applications/Quark.app --args --remote-debugging-port=9222 --remote-debugging-address=127.0.0.1
```

Observed:

- The existing logged-in Kbot page returned `ASTER_QUARK_OK_20261128` during the
  initial probe.
- The final real account test created the client through the normal factory and
  returned `ASTER_QUARK_IMPORT_OK_20261128`.
- A separate real call requested the presented `lookup_weather` tool for 杭州;
  the response contained exactly that tool name and `{"city":"杭州"}`. Aster did
  not execute this fixture tool.
- `cargo run --locked -p aster-search -- "Rust 编程" --engines quark --browser quark --browser-retries 0 --format json --limit 3 --timeout 30`
  returned 3 results and 0 failures after the hydration wait was added. The
  first result was the official Rust Chinese site.
- The real Core `web_search` test also returned a non-empty HTTP(S) result.

## Commands and counts

From the repository root, with the pinned Rust toolchain and initialized
`apps/studio`:

```bash
cargo test --locked -p aster-use-browser --lib
# 23 passed, 0 failed

cargo test --locked -p aster-search --lib
# 560 passed, 0 failed

cargo test --locked -p aster-core --lib llm::quark_account::tests -- --include-ignored
# 7 passed, 0 failed; includes the two explicit live account tests

cargo test --locked -p aster-core --lib config::account_import::tests
# 6 passed, 0 failed

cargo test --locked -p aster-acp --lib quark_import_uses_the_existing_account_catalog_without_a_new_protocol
# 1 passed, 0 failed

cargo test --locked -p aster-core --features headless-search --lib tools::builtin::web_search::tests
# 56 passed, 0 failed, 4 ignored external/soak tests

cargo test --locked -p aster-core --features headless-search --test test_web_search_headless test_quark_browser_search_actual -- --ignored --exact --nocapture
# 1 passed, 0 failed

cargo clippy --locked -p aster-use-browser --all-targets --no-deps
cargo clippy --locked -p aster-search --all-targets --no-deps
cargo clippy --locked -p aster-core --all-targets --no-deps --features headless-search
cargo clippy --locked -p aster-acp --all-targets --no-deps
cargo check --locked -p aster-pager
# All exited 0. Existing unrelated warnings remain.
```

`git diff --check` passed for the affected paths. Scoped `rustfmt --check` was
used after formatting only this task's files.

The full `cargo fmt --all -- --check` remains red because of pre-existing
formatting differences in four unrelated files:

- `crates/code/core/src/sse.rs`
- `apps/studio/src-tauri/src/agent_plane/reasoning_transport_tests.rs`
- `crates/terminal/crates/codegen/aster-pager/src/app/app_view_tests.rs`
- `crates/terminal/crates/codegen/aster-pager/src/app/cli.rs`

Those files were not changed for this task. An initial Core+ACP Clippy invocation
hit its 10-minute timeout; the cached commands were rerun separately and exited
0. No failure is counted as a pass.

## No-UI search requirement

The final search path uses a small scoped CDP client with only
`Page.getFrameTree`, `Network.loadNetworkResource`, `IO.read` and `IO.close`.
It has no browser launch, target creation, navigation or page-close operation.
The default login probe uses the same existing-page connection and submits no
prompt. A missing session or search failure retains the normal fallback.

The live test read the Quark result HTML through
`Network.loadNetworkResource` and asserted that the complete target ID set was
unchanged before and after the call. Core's default-search live test also passed.
The CLI check compared the target set and Quark main-process PID before and after
and required `quark` as the first executed source. A normal API fallback after a
Quark empty/challenge outcome is allowed and is not counted as a UI failure.

## Explicitly not claimed

- Quark does not expose a public account-model API here; the adapter follows the
  Kbot page. A future Quark page redesign may require adapter changes.
- `quark/kbot` is a product route, not a claim about the exact underlying model.
- No native token accounting, attachment transport, native structured-output
  enforcement or provider-enforced output-token cap is claimed.
- The tool fixture proves tool selection/decoding, not execution by Quark or a
  complete multi-turn coding-agent run. Tools are executed only by Aster after
  its normal admission path.
- Windows and Linux runtime behavior was not tested. CDP URL parsing is portable,
  but the only live qualification is the macOS version above.
- No full workspace test suite, release build, publication, Git commit, Studio
  GUI scenario or other pre-existing dirty-worktree verification is claimed.
- Existing unrelated workspace changes were preserved and are not attributed to
  this task.
