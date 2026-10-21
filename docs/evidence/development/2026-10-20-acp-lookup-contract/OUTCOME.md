# ACP lookup contract

## Result

A set but unusable `A3S_ACP_AGENT_BIN` stops selection on AsterDream,
Desktop, and the Aster TUI. An empty value, a missing path, and a Unix
file without an execute bit do not fall through to a sibling, a
bundle, `~/.a3s/bin`, or `PATH`. An executable override is the file
that is selected. When the variable is unset, the TUI accepts only an
executable sibling. Otherwise it returns this existing sentence:

    no agent backend available: build aster and a3s-code-acp together with `cargo build --locked`, or set A3S_ACP_AGENT_BIN to the agent executable

Unset desktop hosts keep their current search order and not-found
sentence. Only a set override requires the execute bit.

A desktop executable outside a Cargo `target/` directory does not
prefer the compile-time `target/debug/a3s-code-acp` over a bundled
agent. An executable under `target/` still prefers the workspace debug
binary when that file exists.

An interactive leader connect failure is reported as that failure.
`start_sidecar` stays false for both a failed connect and a cancelled
one. `aster agent leader` still calls `run_leader`, which builds
`MvpAgent::with_models`. `aster agent serve` still calls
`run_agent_server`, which builds `MvpAgent::new`. Neither entry was
moved onto `a3s-code-acp`.

Sidecar initialize capability meta has no key that starts with
`x.ai/`. Leader initialize still advertises exactly
`x.ai/bashOutputNoColor`, `x.ai/gitHeadChanged`, `x.ai/hunkTracker`,
`x.ai/incrementalBashOutput`, `x.ai/statusLine`, and
`x.ai/userMessageEcho`. `crates/code/acp` does not implement those six
keys. `specs/002-shared-acp-agent/spec.md` records both paths.

`spawn_grok_shell` is not defined and not called in the pager crate.
`spawn_runtime_thread` and `boot_auth_manager` remain.
`spawn_agent_thread_direct`, `spawn_skills_file_watcher`, and
`apply_discovery_change` had no remaining callers and were removed
with `spawn_grok_shell`. The product shell-spawn set stays empty.

## Commands and counts

- `cargo test --locked --offline -p a3s-code-pager --lib -- a_set_override_stops_when_the_path_cannot_be_executed an_unset_override_accepts_only_an_executable_sibling a3s_acp_executable_reads_the_environment_override headless_unusable_override_uses_the_interactive_error headless_missing_sidecar_uses_the_interactive_error`
  — 5 passed, 10181 filtered, 3.68s. Exit 0. The two missing-sidecar
  tests assert the sentence above.
- `cargo test --locked --offline -p aster-dream --lib -- unusable_override_does_not_select_a_later_candidate find_binary_rejects_a_set_but_unusable_override requested_agent_bin_is_the_one_that_launches explicit_agent_bin_wins_over_the_workspace_build published_release_is_used_when_the_workspace_build_is_absent standard_install_is_found_without_a_shell_path legacy_install_is_found_without_a_shell_path path_lookup_uses_the_platform_executable_and_path_order workspace_debug_build_wins_over_a_newer_published_release`
  — 9 passed, 854 filtered, 0.01s. Exit 0.
- `cargo test --locked --offline -p a3s-desktop --lib -- unusable_override_does_not_select_a_later_candidate find_binary_rejects_a_set_but_unusable_override requested_agent_bin_is_the_one_that_launches explicit_agent_bin_wins_over_the_workspace_build published_release_is_used_when_the_workspace_build_is_absent bundled_release_wins_when_the_checkout_is_absent standard_install_is_found_without_a_version_suffix path_lookup_uses_an_extra_directory_when_path_is_empty workspace_debug_build_wins_over_a_newer_published_release`
  — 9 passed, 811 filtered, 0.01s. Exit 0.
- `cargo test --locked --offline -p aster-dream --lib -- packaged_host_does_not_prefer_the_compile_time_debug_binary`
  — 1 passed, 862 filtered. Exit 0.
- `cargo test --locked --offline -p a3s-desktop --lib -- packaged_host_does_not_prefer_the_compile_time_debug_binary`
  — 1 passed, 819 filtered. Exit 0.
- `cargo test --locked --offline -p a3s-code-pager --lib -- leader_connect_failure_reports_that_failure_without_a_sidecar`
  — 1 passed, 10185 filtered. Exit 0.
- `cargo test --locked --offline -p a3s-code-pager --lib -- initialize_xai_advertisements_are_the_closed_set sidecar_initialize_omits_xai_keys client_capabilities_meta_defaults_absent_or_blank_mode_to_off client_capabilities_meta_always_requests_user_message_echo client_capabilities_meta_advertises_the_status_line_the_config_asked_for client_capabilities_meta_canonicalizes_off_and_mixed_case`
  — 6 passed, 10180 filtered, 0.01s. Exit 0.
- `cargo test --locked --offline -p a3s-code-pager --lib -- acp::spawn::`
  — 10 passed, 10176 filtered, 4.07s. Exit 0. Test profile finished in
  1m 03s.
- `python3 -m unittest tests.agent_runtime_boundary -v` — 23 tests,
  OK, 0.169s. Exit 0. `pager_shell_spawn_files` is `[]`.
- A search of the pager crate for `spawn_grok_shell` found no matches.

## Explicitly not claimed

- The live LLM prompt was not re-run. Boyue is not a pass.
- No ACL or TOML setting was added. No shared lookup crate was added.
- The six `x.ai/` keys were not implemented in the ACP agent.
- Pager layout, crate names, versions, editions, and licenses were not
  changed.
- Nothing was committed.
