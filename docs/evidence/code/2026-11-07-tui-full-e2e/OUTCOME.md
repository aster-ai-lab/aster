# OUTCOME — Aster TUI full E2E (2026-11-07)

Implements the plan against [specs/023-tui-core-acp-full-e2e](../../../specs/023-tui-core-acp-full-e2e/spec.md).
Matrix: [FEATURE-MATRIX.md](FEATURE-MATRIX.md). Artifacts under `artifacts/`.

## Pair identity (Campaign A)

```text
cargo build --locked -p aster -p aster-acp
```

| Field | Value |
| --- | --- |
| host | Darwin arm64 |
| rustc | 1.97.1 |
| aster | 0.1.9 (a2fdfd625beb) |
| aster-acp | 9.1.8 |
| pair stamp | `aster-cli-pair-v1:bdf7db9857beee3638b283894dc33d94dd4a24e1:aarch64-apple-darwin` (both binaries) |
| aster sha256 | fe8a3669095f6f461993ae2e67840e357c2c286ad8282e8a3f2248bb39ccf1e6 |
| acp sha256 | 2a227417e6a9ef7970aca6feaf046910abc4fd3ba6580505705a408eeb8bca51 |

Record: `artifacts/final-pair.txt`, `artifacts/pair-build.log`.

## Campaign B — Hermetic

### Commands and counts

| Suite | Command | Result |
| --- | --- | --- |
| shipped_tui_e2e (full) | `cargo test --locked -p aster-acp --test shipped_tui_e2e -- --test-threads=1` | **18 passed**, 0 failed, **3 ignored** |
| launch/resume ORACLEs | same filter + `--nocapture` | **100+ ORACLE pass** lines in `artifacts/oracles.txt` |
| tui_913_hermetic | `cargo test --locked -p aster-acp --test tui_913_hermetic -- --test-threads=1` | **7 passed**, 0 failed, **2 ignored** (need `ASTER_SANDBOX_DIR`) |
| tui_one_agent | `cargo test --locked -p aster --test tui_one_agent -- --test-threads=1` | **1 passed** |

Ignored (not counted as pass):

- `shipped_native_form_submit_cancel_and_reject` — requires `aster-test` (not on PATH)
- `retrieval::shipped_retrieval_aster_test_terminal_round_trip` — requires `aster-test`
- `shipped_bound_goal_real_llm` — live config pin (Campaign C separate)
- `three_turns_*` / `long_task_*` — macOS needs explicit qualified `ASTER_SANDBOX_DIR` (in-tree sandbox externalized)

Form submit/cancel/reject remains covered by stdio/native paths only when aster-test is present; `ask-form.acl` not executed here.

### ORACLE coverage highlights

From `shipped_tui_launch_resume_and_visible_commands` (`artifacts/oracles.txt`):

- Session: continue/resume durable transcript, new/home/delete/quit
- Slash effects: settings, config, model, effort, goal*, compact, context, jump, find, export, copy, theme, yolo, vim, multiline, compact-mode, timestamps, mouse, minimal/fullscreen, timeline, doctor, help, expand, history, /memory inspect, /tasks inspect
- Permissions: ask-allow-once-gate, yolo side-effect
- Refuse-ok: extensive `hidden-*` palette absence (usage, plan, login, plugins, skills, mcps, workflows, …)

Other hermetic passes: session wire, identity to model, permission reject/cancel, prefix fork, config failure before connect budget, hung-agent bounded wait, retrieval stdio (3), form value prefix survival, readiness unit tests, one-agent pair connect, hermetic one-turn / model / missing-provider / version.

## Campaign C — Live (independent)

Operator ACL used only via process env / temp rewrite of `default_model` (no credentials copied into this evidence tree).

| Case | Model | Result |
| --- | --- | --- |
| `real_llm_tui_turn_shows_assistant_text` | `deepseek/deepseek-flash` | **pass** — `ORACLE case=real-llm … final_reply_visible=true` |
| same (attempt) | `kimi/k3.1-pro` | **blocked** — monthly usage limit banner |
| `real_llm_tui_cancel_active_shell_and_recover` via `acl_env_config run` | `deepseek/deepseek-flash` | **pass** — cancelled, new session, uncertainty retained, read-after-cancel |

Logs: `artifacts/live-one-turn-deepseek.log`, `artifacts/live-cancel-recover.log`, `artifacts/live-result.txt`.

Live multi-turn coding (`three_turns` / dual-defect) and bound-goal real_llm were **not** re-run in this campaign.

## FR / SC audit (023)

| ID | Verdict |
| --- | --- |
| FR-001 pair build/identity | met (matching pair stamp) |
| FR-002 complete matrix inventory | met as FEATURE-MATRIX.md; gaps remain labeled |
| FR-003 owning hermetic regressions | met for exercised shipped/hermetic suites |
| FR-004 every feature family via real TUI/ACP | **partial** — baseline slash/session/tools covered; advanced host methods and aster-test ACL journeys open |
| FR-005 live-model campaign | **partial** — one-turn + cancel/recover; not full coding matrix |
| FR-006 ACL/trust/sandbox/APIs | preserved; no oracle branches added |
| FR-007 overfitting audit | no product changes in this campaign |
| FR-008 evidence retained | this OUTCOME + artifacts |
| SC-001 identities | met |
| SC-002 matrix no incomplete required row | **not met** — rows remain `gap` / `blocked` |
| SC-003 PTY + ACP cases | met for executed hermetic set |
| SC-004 live coding | **partial** (prose + cancel only) |
| SC-005 fmt/workspace guards | not re-run as part of this campaign |
| SC-006 completion audit | open until gap rows closed or explicitly waived |

## Explicitly not claimed

- Studio / desktop CUA acceptance
- Cross-Prompt background subagent contract
- Full aster-test ACL suite execution (`aster-test` / `a3s-test` absent from PATH)
- Native form PTY ACL (`shipped_native_form_*` ignored)
- Hermetic multi-turn/long-task without `ASTER_SANDBOX_DIR`
- Live kimi/k3.1-pro (quota exhausted); live dual-defect coding gate
- Advanced unconfigured SDK caps beyond palette refuse-ok (packages, state_graph, s3, otel, …)
- Host methods without named TUI e2e (review, wiki, interject, background-tasks stop, …)
- Tool gaps: git, download, generate_object, per-mode search, task_control kill/get_output
- `aster-pager-pty-harness` ignored PTY matrix (~177 modules / YAML scenarios) — outside default CI; not executed this campaign
- Manual 1,020-case inventory under `crates/code/manual/tui-e2e/20261012`

## Follow-up when `aster-test` is installed

Canonical hermetic ACL starter (from inventory; not run here):

```bash
cargo build --locked -p aster -p aster-acp
python3 tests/e2e/startup.py
# then: ask-form.acl / workspace-retrieval.acl via the same binary + isolated HOME
# cargo test -p aster-acp --test shipped_tui_e2e shipped_native_form_submit_cancel_and_reject -- --ignored --exact --nocapture
```

UI-mechanics depth beyond shipped pair E2E: opt into `aster-pager-pty-harness` with `--ignored --test-threads=1` (mock inference), separately from live LLM gates.

## Process cleanup

Cargo test runners completed; no intentional orphan PTY left by this campaign. Live temp ACL files under `/tmp/aster-live-*.acl` removed after runs. Re-checked PATH for `aster-test`/`a3s-test` after inventory agents; still absent, so ACL unblock was not possible in-session.

## Retirement (same day follow-up)

Removed in-repo A3S/`aster-test` TUI surface: `tests/e2e/*` TUI ACL suites and
Python launchers, `crates/code/acp/tests/aster-test/`,
`long_horizon_tui_real_llm` / `tui_lifecycle`, and the ignored
`shipped_native_form_*` / `shipped_retrieval_aster_test_*` drivers. TUI E2E
continues via Rust `portable_pty` suites only. GUI `surface = "gui"` ACL and
`aster-test-support` (mock inference harness) are unrelated and retained.

## DeepSeek live campaign (same day)

Source: `./.aster/config.acl` with launch-only `default_model = "deepseek/deepseek-flash"`
(temp ACL; credentials not copied into evidence).

| Case | Result |
| --- | --- |
| `real_llm_tui_turn_shows_assistant_text` | pass |
| `real_llm_tui_cancel_active_shell_and_recover` | pass |
| `real_llm_tui_turns_fix_the_diagnosed_bug` | pass |
| `real_llm_tui_long_task_repairs_and_quotes_the_check` | pass |

Artifacts: `artifacts/deepseek-*.log`, `artifacts/deepseek-campaign-result.txt`.
