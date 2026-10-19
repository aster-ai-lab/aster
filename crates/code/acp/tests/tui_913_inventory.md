# Shipped TUI case inventory

## Offered Aster TUI behaviors

Each row is driven by the shipped `aster` binary and its ACP companion against a loopback model and an isolated home. Palette presence is not the pass record; the named oracle is the primary observable.

| Behavior | Test | Pass record |
| --- | --- | --- |
| Welcome on a wide terminal shows Aster | `tests/e2e/startup.py` (`tests/e2e/startup.acl`) | viewport 120×40 contains Aster |
| Welcome on a narrow terminal shows Aster and the version | `tests/e2e/startup.py` (`tests/e2e/startup.acl`) | viewport 60×20 contains Aster and `0.1.0` |
| Welcome frame at the standard size | `tests/e2e/startup.py` (`tests/e2e/startup.acl`) | viewport 80×24 contains Aster |
| Cold launch stays on the ACL model | `shipped_tui_launch_resume_and_visible_commands` | `continue-same-session` model `mock/gpt` |
| `--continue` reopens that session and its transcript | `shipped_tui_launch_resume_and_visible_commands` | `continue-durable-transcript` |
| `--resume` reopens that session and its transcript | `shipped_tui_launch_resume_and_visible_commands` | `resume-durable-transcript` |
| One local-model turn renders the assistant reply | `one_turn_shows_assistant_text` | `ORACLE case=turn` |
| `/settings` opens the settings modal | `shipped_tui_launch_resume_and_visible_commands` | `settings` |
| `/config` opens providers and models | `shipped_tui_launch_resume_and_visible_commands` | `config` |
| `/new` opens a different session | `shipped_tui_launch_resume_and_visible_commands` | `new` |
| `/effort` changes the thinking budget and tool rounds | `shipped_tui_launch_resume_and_visible_commands` | `effort` |
| `/goal` sets, pauses, resumes, and clears a durable goal | `shipped_tui_launch_resume_and_visible_commands` | `goal-status`, `goal-set`, `goal-pause`, `goal-resume`, `goal-clear` |
| `/compact` reports an empty session, then compacts a real one | `shipped_tui_launch_resume_and_visible_commands` | `compact-empty`, `compact-done` |
| `/model` persists another catalog model | `shipped_tui_launch_resume_and_visible_commands` | `model` |
| `/context` reports turns and tokens | `shipped_tui_launch_resume_and_visible_commands` | `context` |
| `/jump` previews a prior turn | `shipped_tui_launch_resume_and_visible_commands` | `jump-turn` |
| `/edit-prompt` launches the editor | `shipped_tui_launch_resume_and_visible_commands` | `edit-prompt` |
| `/session-info` shows the session, turn, and tokens | `shipped_tui_launch_resume_and_visible_commands` | `session-info` |
| `/export` writes the transcript | `shipped_tui_launch_resume_and_visible_commands` | `export` |
| `/copy` writes the assistant reply | `shipped_tui_launch_resume_and_visible_commands` | `copy` |
| `/find` searches the scrollback | `shipped_tui_launch_resume_and_visible_commands` | `find-marker` |
| `/theme` persists the theme and does not call the model | `shipped_tui_launch_resume_and_visible_commands` | `theme` |
| `/yolo` skips the prompt and creates a new side-effect file | `shipped_tui_launch_resume_and_visible_commands` | `yolo` |
| `/vim-mode` persists vim mode | `shipped_tui_launch_resume_and_visible_commands` | `vim-mode` |
| `/multiline` toggles the session flag | `shipped_tui_launch_resume_and_visible_commands` | `multiline-on`, `multiline-off` |
| `/compact-mode` persists compact mode | `shipped_tui_launch_resume_and_visible_commands` | `compact-mode` |
| `/timestamps` persists the timestamp flag | `shipped_tui_launch_resume_and_visible_commands` | `timestamps` |
| `/toggle-mouse-reporting` changes mouse capture | `shipped_tui_launch_resume_and_visible_commands` | `toggle-mouse-reporting` |
| `/minimal` switches mode | `shipped_tui_launch_resume_and_visible_commands` | `minimal` |
| `/timeline` persists the sidebar | `shipped_tui_launch_resume_and_visible_commands` | `timeline` |
| `/doctor` reports the terminal and does not call the model | `shipped_tui_launch_resume_and_visible_commands` | `doctor` |
| `/home` returns to the welcome screen with no active session | `shipped_tui_launch_resume_and_visible_commands` | `home` |
| `/delete` removes the session | `shipped_tui_launch_resume_and_visible_commands` | `delete` |
| `/help` opens the command list and does not call the model | `shipped_tui_launch_resume_and_visible_commands` | `help` |
| `/quit` exits 0 | `shipped_tui_launch_resume_and_visible_commands` | `quit` |
| `/expand` reprints folded tool output in minimal mode | `shipped_tui_launch_resume_and_visible_commands` | `expand-marker` |
| `/fullscreen` returns from minimal mode | `shipped_tui_launch_resume_and_visible_commands` | `fullscreen` |
| Form required-field rejection | `shipped_native_form_submit_cancel_and_reject` | `empty-field` `required` |
| Form limit rejection (`max 5`) | `shipped_native_form_submit_cancel_and_reject` | `range` `max 5` |
| Form submit | `shipped_native_form_submit_cancel_and_reject` | `FORM-SUBMITTED` |
| Form cancel | `shipped_native_form_submit_cancel_and_reject` | `FORM-CANCELLED` |
| Form reject | `shipped_native_form_submit_cancel_and_reject` | `FORM-REJECTED` |
| Allow once performs the write | `shipped_tui_launch_resume_and_visible_commands` | `ask-allow-once-gate` |
| Reject leaves the write unrun | `shipped_tui_permission_reject_and_cancel_skip_the_side_effect` | `permission-reject` |
| Cancel leaves the write unrun | `shipped_tui_permission_reject_and_cancel_skip_the_side_effect` | `permission-cancel` |

## Absent from the fullscreen session

These names stay out of the agent palette. The pass record is `hidden-<name>` absent, from `shipped_tui_launch_resume_and_visible_commands`. They are not features to implement.

| Name | Status |
| --- | --- |
| usage | absent |
| view-plan | absent |
| plan | absent |
| tutorial | absent |
| docs | absent |
| privacy | absent |
| login | absent |
| logout | absent |
| btw | absent |
| feedback | absent |
| queue | absent |
| fork | absent |
| resume | absent |
| rename | absent |
| history | absent |
| transcript | absent |
| rewind | absent |
| workflows | absent |
| workflow | absent |
| personas | absent |
| config-agents | absent |
| plugins | absent |
| skills | absent |
| mcps | absent |
| hooks | absent |
| marketplace | absent |
| import-claude | absent |
| remember | absent |
| tasks | absent |
| imagine | absent |
| imagine-video | absent |
| voice | absent |
| dashboard | absent |
| recap | absent |
| share | absent |
| release-notes | absent |
| announcements | absent |
| dream | absent |
| flush | absent |
| memory | absent |
| loop | absent |
| auto | absent |
| cd | absent outside the dashboard (`cd-agent-palette`) |

## Other hermetic rows

These rows are outside the offered-surface gate above. Real-provider and multi-turn coding rows stay here and are not part of that run.

The interactive session is `a3s-code-tui` speaking ACP to `a3s-code-acp`.
Both binaries must report the current ACP package version. Hermetic rows use a local OpenAI-compatible
server and do not call a live provider. The real-LLM row is a separate
test binary so a missing provider cannot hide a launch failure.

| Case | Test | Binary |
| --- | --- | --- |
| launch | `composer_opens_on_the_acl_default_model` | `tui_913_hermetic` |
| model selection | `model_command_selects_the_catalog_model` | `tui_913_hermetic` |
| one conversational turn | `one_turn_shows_assistant_text` | `tui_913_hermetic` |
| missing-provider refusal | `missing_provider_refuses_without_an_invented_reply` | `tui_913_hermetic` |
| 轮次 | `three_turns_fix_the_bug_they_diagnosed` | `tui_913_hermetic` |
| 长程任务 | `long_task_repairs_the_check_after_a_failure` | `tui_913_hermetic` |
| one real-LLM turn | `real_llm_tui_turn_shows_assistant_text` | `tui_913_real_llm` |
| real-LLM 轮次 | `real_llm_tui_turns_fix_the_diagnosed_bug` | `tui_913_real_llm` |
| real-LLM 长程任务 | `real_llm_tui_long_task_repairs_and_quotes_the_check` | `tui_913_real_llm` |
| Cursor account text and picker | `picker_shows_the_cursor_account_and_a_turn_returns_stub_text` | `cursor_account_tui_e2e` |
| Cursor real model read/write and host byte verification | `live_cursor_tools_read_write_and_verify_actual_workspace_bytes` (explicit `--ignored`; fixture-only disk content attestor) | `cursor_account_tui_e2e` |
| Cursor real TUI reads, writes and refuses unverified completion | `tests/e2e/cursor_tools.acl` (prepared isolated Cursor config/workspace; assert output bytes after run) | `a3s-test` |

`shipped_binaries_report_current_version` in the hermetic binary checks the two
version lines. The real-LLM tests run only when `A3S_CONFIG_FILE` is
set; they are not `#[ignore]`. Each points `A3S_CONFIG` at that same file
and does not rewrite it.

The one-turn real-LLM test (`real_llm_tui_turn_shows_assistant_text`) is
non-empty assistant text. It is not the 轮次 gate. The one-defect
subtraction task (`calc.py` `return a - b`, token `SUM-OK-9`) is retired
as the gate.

轮次 keeps one pager session. `rate.py` and `fee.py` start with two
independent defects. The diagnosis names both and edits nothing. The next
prompt does not restate either defect and leaves exactly one; `gate.py`
still prints `DUAL-HOLD`. A later prompt does not restate the remaining
defect and clears both. The last prompt quotes the checker line. That
prompt does not contain `DUAL-GATE-7K`. A screen line that contains the
token is not enough, and a new `runs.txt` line is not enough by itself.
The quote wait does not read the screen. It ends only when `runs.txt`
gains a line equal to `DUAL-GATE-7K` and the session tool trace shows a
`python3 gate.py` command whose stdout has a line equal to that token.
A screen sentence, a `runs.txt` sentence that only contains the token,
a hand-written append, and a run that sets `GATE_SKIP_LOG` do not count. `gate.py` does not mention
`GATE_SKIP_LOG`. The test's own checker runs in a private directory and
does not write the session `runs.txt`. The cargo log prints `runs.txt`
before the success token, then prints the bash call id and that traced
command. The call id is not in the user prompt. The log also prints the
tool stdout line. It does not print the model's surrounding sentence.
The screen shows that token
only after a model request in the session already carried `DUAL-HOLD`.
`gate.py` bytes stay unchanged. Fixing only one defect, or swapping one
operator, still fails the checker.

长程任务 is one prompt. It contains neither `DUAL-GATE-7K` nor the defect
text. The session must see the checker fail, correct one file, see the
checker still fail, correct the other file, and quote `DUAL-GATE-7K`.
The hermetic run makes at least 6 model requests. A request made while
exactly one defect remains still contains `DUAL-HOLD`, and that output
appears before any request contains `DUAL-GATE-7K`. Each real `python3
gate.py` execution appends its printed line to `runs.txt`. The long-task
order oracle is that file: at least two `DUAL-HOLD` lines before the
first `DUAL-GATE-7K`, and one of those `DUAL-HOLD` lines must be appended
while exactly one defect remains. Two failing runs before any edit do
not satisfy that. The test's own checker runs in a private directory and
does not write that file. The checker source does not mention
`GATE_SKIP_LOG`. The 轮次 and 长程任务 tests panic when
`A3S_CONFIG_FILE` is unset. The one-turn test still returns early.

The CLI crate pins Core 9.1.4 (`apps/cli`). Its release pager and ACP use the
same publicly fetchable Code revision; local development builds can be newer.
