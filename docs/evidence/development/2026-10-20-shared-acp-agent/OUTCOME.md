# Shared ACP agent

## Result

Headless, `aster agent stdio`, `models`, and `worktree` start the
same `a3s-code-acp` sidecar as the interactive pager. A missing
executable returns `MISSING_AGENT_BACKEND`. The product shell-spawn
file set is empty. The six initialize `x.ai/` advertisements stay a
closed client set. An ordinary prompt meta sends no `x.ai/` key.

The live prompt gate passed on the official DeepSeek provider in the
repo `.a3s/config.acl`. Two consecutive runs started the shipped
`a3s-code-acp` process against `deepseek/deepseek-flash` and each
printed `LIVE_LLM reply_nonempty=1 usage=16334 model=deepseek/deepseek-flash reply_len=4 preview=pong`.
Run 1 finished in 7.28s, exit 0. Run 2 finished in 6.15s, exit 0.
A third run, with the test default set to that same model, finished
in 6.81s, exit 0, and printed the same line. All three are inside
180 seconds. The assistant reply is the single word `pong`.

Earlier runs against `boyue/deepseek-v4-flash` stopped on
`boyue API account has no balance (HTTP 401)` before any assistant
text or usage tokens. The client maps a boyue HTTP 401 body of
`Invalid token` (`new_api_error`) to that sentence. A direct request
to the configured boyue base URL returns that body. The repo ACL, the
user ACL, and the other local provider keys on that host return the
same rejection. Those boyue prompts were not repeated. Scripted
conformance is not the live run.

A later search found no second boyue credential. The CC Switch
database has no boyue provider. Desktop `providers.json`, `boyue.env`,
the repo ACL, and the user ACL share one boyue key. The console
supplier record decrypts to that same key. Browser stores and the
login keychain have no other key for that host. One other local
supplier key, issued for a different host, is also rejected by this
gateway as `Invalid token`. Running `a3s-code-acp` processes point
`A3S_CONFIG` at an ACL with no provider key. Git history has no copy
of the repo ACL. Editor local history of those ACL files keeps the
same boyue key. A fresh Desktop `aster/providers.json` still stores
that same boyue key. Two other keys in that file, issued for other
hosts, also return `Invalid token` from this gateway. The two live
prompts were not repeated.

## What landed

- `specs/002-shared-acp-agent/` — switch order is headless, then
  stdio, then `models` and `worktree` together. Research classifies
  the six initialize keys as client advertisements. The shared agent
  does not read them. The client does not depend on an agent
  implementation. Ordinary `PromptRequest` meta is `promptId` and
  optional `screenMode`.
- `acp/a3s_spawn.rs` — `spawn_required_sidecar` checks
  `a3s_acp_enabled()` before `spawn_a3s_acp`. `run_sidecar_on_stdio`
  uses the same check. The missing sentence is `MISSING_AGENT_BACKEND`.
- Interactive `acp/mod.rs`, `headless.rs` `spawn_agent`, `models.rs`,
  and `worktree_cmd/mod.rs` call `spawn_required_sidecar`.
  `agent_runtime.rs` `run_stdio` calls `run_sidecar_on_stdio`.
- `tests/agent_runtime_boundary.json` — `pager_shell_spawn_files` is
  `[]`. Stdio must contain `run_sidecar_on_stdio(` and must not
  contain `run_stdio_agent(`. The sidecar gate body order is locked.
  The leader cluster file must still name `MvpAgent` and must not call
  the sidecar.
- Hermetic tests: `headless_missing_sidecar_uses_the_interactive_error`,
  `initialize_xai_advertisements_are_the_closed_set`,
  `ordinary_prompt_meta_sends_no_xai_extension`.
- `crates/code/acp/tests/live_shared_prompt_real_llm.rs` — ignored
  unless `--ignored` is passed. It does not set `HOME`. It passes
  `--config` and `A3S_CONFIG`.

## Commands and counts

- `python3 -m unittest tests.agent_runtime_boundary -v` — 23 tests,
  OK, 0.155s. Exit 0.
- `cargo test --locked --offline -p a3s-code-pager --lib -- headless_missing_sidecar_uses_the_interactive_error initialize_xai_advertisements_are_the_closed_set ordinary_prompt_meta_sends_no_xai_extension`
  — 3 passed, 10177 filtered, 17.47s. Test profile finished in 4m 54s.
  Exit 0.
- `cargo test --locked --offline -p a3s-code-acp --test acp_conformance_stdio`
  — 1 passed, 4.89s. Test profile finished in 7m 24s. Exit 0. The spec
  required no new ACP method. One earlier attempt on a loaded machine
  timed out the first two initialize cases; this count is the rerun.
- `cargo test --locked --offline -p a3s-code-acp --test live_shared_prompt_real_llm -- --ignored --exact shared_acp_live_prompt_returns_usage --nocapture`
  — earlier boyue runs: run 1 failed in 8.30s, exit 101; run 2 failed
  in 5.30s, exit 101. Both errors are
  `boyue API account has no balance (HTTP 401)`. Neither transcript
  contains `LIVE_LLM`.
- The same command against `deepseek/deepseek-flash` from the repo
  `.a3s/config.acl`: run 1 passed in 7.28s, exit 0; run 2 passed in
  6.15s, exit 0; the default-model rerun passed in 6.81s, exit 0.
  Each printed `LIVE_LLM reply_nonempty=1 usage=16334 model=deepseek/deepseek-flash reply_len=4 preview=pong`.
  `A3S_CONFIG_FILE` was unset. The test used the repo `.a3s/config.acl`.

## Explicitly not claimed

- The passing live runs use the official `deepseek` provider at
  `https://api.deepseek.com`. They do not exercise the boyue gateway.
  Conformance was not treated as the live run.
- The leader cluster still uses in-process `MvpAgent`.
- Pager layout and license notices were not rewritten.
- `SessionFlags::to_meta` is session create and load, not the ordinary
  prompt. Inbound `x.ai/session/update`, `x.ai/session_notification`,
  and the agent host methods were not reclassified.
- `event_loop.rs` was not split. Its inline tests stayed.
- No new ACL or TOML setting. `A3S_REFUSE_UNQUALIFIED_NATIVE_SHELL`
  was not copied into ACP. OCI remains the product default.
- Crate names, version, edition, and license were not unified.
- No Node, Python, or Go bindings. No GUI. `cargo test -p aster` was
  not run. Nothing was committed.
- Desktop delivery-alignment files were not edited.
- Core `both_native_apis_refuse_uniformly_without_effects_or_output`
  was not re-run.
