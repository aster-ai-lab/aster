# Aster TUI Feature Matrix — 2026-11-07

Source closed sets: Core `sdk_capabilities.rs` (30), ACP `commands.rs` /
`host_methods.rs`, pager `slash/commands` builtins. Status is evidence from
this campaign only ([OUTCOME.md](OUTCOME.md)).

Legend: `pass` | `refuse-ok` | `gap` | `blocked` | `pending`

## A. SDK capability groups

| Item | Status | Evidence |
| --- | --- | --- |
| agent_runtime | pass | pair stamp; tui_one_agent; session wire |
| conversation | pass | hermetic one_turn; live one-turn ORACLE |
| run_control | pass | live cancel-recover ORACLE; hermetic permission cancel |
| governed_tools | pass | ask-allow-once; permission reject/cancel |
| workspace_tools | pass | write/bash oracles; session wire |
| workspace_retrieval | pass | shipped_retrieval_stdio_* (3) |
| model_adapters | pass | /model mock/alt; xai key keeps ACL model |
| structured_output | gap | no named TUI e2e |
| mcp_and_skills | refuse-ok | hidden-skills / hidden-mcps palette |
| planning_delegation | gap | live plan/task caps not re-run this campaign |
| priority_scheduling | gap | — |
| persistence | pass | continue/resume/fork oracles |
| governance | pass | permission reject/cancel; yolo |
| run_observability | pass | session-info / context oracles (partial surface) |
| context_memory | refuse-ok | /memory inspect only; manage ops not proven |
| web_search | gap | real_llm cap not re-run |
| web_fetch | gap | real_llm cap not re-run |
| program | gap | real_llm cap not re-run |
| code_intelligence | gap | real_llm cap not re-run |
| packages | gap | unconfigured refuse path not separately asserted |
| use_runtime_tasks | gap | — |
| programmable_workflows | refuse-ok | hidden-workflow(s) |
| state_graph | gap | — |
| agent_release_contract | gap | — |
| agent_protocol | gap | — |
| evaluation_substrate | blocked | out of TUI agent surface |
| typed_decisions | gap | — |
| moli_runtime | gap | — |
| s3_workspace | gap | — |
| opentelemetry | gap | — |

## B. Tool families

| Item | Status | Evidence |
| --- | --- | --- |
| read / ls | pass | refuses_repeated_reads; live read-after-cancel |
| write | pass | ask-allow-once; yolo side-effect; permission reject/cancel |
| edit / patch / download | gap | — |
| bash / background | pass | wire; live cancel shell |
| search (hybrid/stdio) | pass | shipped_retrieval_stdio_* |
| search (per-mode) | gap | — |
| git | gap | — |
| web_fetch / web_search | gap | not re-run live |
| update_plan | gap | not re-run live |
| ask_user / ask_user_form | gap | A3S TUI ACL retired; no replacement PTY form suite yet |
| batch | gap | tl01 not re-run |
| program / task | gap | not re-run live |
| code_symbols | gap | not re-run live |
| code_navigation / diagnostics | gap | — |
| get_task_output / kill_task | gap | — |
| generate_object | gap | — |

## C. ACP commands / host methods

| Item | Status | Evidence |
| --- | --- | --- |
| /goal | pass | goal-set/pause/resume/clear oracles |
| /memory inspect | pass | zero model requests oracle |
| /memory manage | gap | — |
| /tasks inspect | pass | zero model requests oracle |
| /tasks stop | gap | — |
| help/compact/model/history | pass | shipped oracles |
| tools/mcp/cost | gap / refuse-ok | cost≈usage hidden |
| aster/goal/* | pass | goal oracles + wire |
| aster/compact_conversation* | pass | compact-done |
| aster/session/{info,list,delete} | pass | session-info; delete |
| aster/session/fork/prefix/v1 | pass | prefix_fork test |
| aster/models/list | gap | indirect only |
| aster/followup/generate | gap | — |
| aster/goal/import,preview | gap | — |
| aster/review/* | gap | — |
| aster/wiki/search | gap | — |
| aster/workspace/search/v1 | gap | retrieval related, host method not named |
| aster/memory/manage,snapshot | gap | — |
| aster/interject* | gap | — |
| aster/permission_posture | gap | A3S TUI ACL retired |
| aster/session/transcript/page/v1 | gap | — |
| aster/session/tooling/v1 | gap | — |
| aster/session/background-tasks* | gap | — |
| aster/subagent/cancel | gap | — |
| unknown host method | refuse-ok | unit only (host_methods_tests) |

## D. Pager slash — offered

All inventory offered commands exercised in
`shipped_tui_launch_resume_and_visible_commands` → **pass** (see
`artifacts/oracles.txt`): settings, config, new, effort, goal*, compact, model,
context, jump, edit-prompt, session-info, export, copy, find, theme, yolo,
vim-mode, multiline, compact-mode, timestamps, toggle-mouse-reporting, minimal,
fullscreen, timeline, doctor, home, delete, help, quit, expand, fork, history,
memory inspect, tasks inspect.

## E. Pager slash — unavailable

`hidden-*` ORACLEs → **refuse-ok** for: usage, view-plan, plan, tutorial, docs,
privacy, login, logout, btw, feedback, queue, resume, rename, transcript, rewind,
workflows, workflow, personas, config-agents, plugins, skills, mcps, hooks,
marketplace, import-claude, remember, imagine, imagine-video, voice, dashboard,
recap, share, release-notes, announcements, dream, flush, loop, auto, cd
(off dashboard).

## F. Launch / session

| Item | Status | Evidence |
| --- | --- | --- |
| Welcome / resize ACL | gap | A3S TUI ACL retired; use Rust PTY welcome coverage |
| Companion / config failure | pass | shipped_tui_reports_configuration_failure_* |
| Hung agent wait | pass | shipped_tui_keeps_a_bounded_wait_* |
| continue / resume | pass | continue-*/resume-* ORACLEs |
| Missing provider | pass | missing_provider_refuses_* |
| One-agent pair | pass | tui_one_agent |
| Multi-turn / long-task hermetic | blocked | needs ASTER_SANDBOX_DIR |
| Live one-turn | pass | deepseek/deepseek-flash |
| Live cancel/recover | pass | acl_env_config + deepseek |
| Live kimi pin | blocked | provider monthly quota |
| Cursor tools ACL | gap | A3S TUI ACL retired; cursor_account_tui_e2e remains |

## Rollup (this campaign)

| Bucket | pass | refuse-ok | gap | blocked |
| --- | ---: | ---: | ---: | ---: |
| SDK caps | 10 | 3 | 16 | 1 |
| Tools | 5 | 0 | 10 | 1 |
| Host/cmd | ~10 | 1 | ~15 | 0 |
| Pager offered | ~32 | 0 | 0 | 0 |
| Pager hidden | 0 | ~40 | 0 | 0 |
| Launch/live | 7 | 0 | 0 | 4 |
