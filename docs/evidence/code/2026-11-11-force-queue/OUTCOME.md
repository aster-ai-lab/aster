# Foreground cancellation with two waiting prompts

The source-reconciled CLI and ACP passed three full prompt sequences. Each
started a real foreground Bash process, queued two distinct follow-ups, then
forced a separately marked replacement. The original process was cancelled
and the replacement ran before the first and second waiting prompts. Each
sequence retained one cancelled Run and three completed Runs.

## Native DeepSeek

The owned macOS Terminal window used the configured
`deepseek/deepseek-flash` model. Agent-desktop captured the original running
tool and both waiting rows, physically delivered `Ctrl+O`, then captured
the settled window. Verified native keystrokes filled the composer. The
original Run recorded its cancelled terminal 589 ms after gesture dispatch
began. Its original prepared Bash invocation and cancelled receipt shared
one identity. The receipt retained exit code 137 and complete effect
observation. The fixture process was gone and its delayed marker was absent.

The replacement, first waiting prompt and second waiting prompt each
completed once, in that order. Their persisted admission times followed
the previous Run's terminal time. The four prior Run IDs were retained and
the store contained exactly four new Run IDs. There were no queued rows or
busy indicators after settlement. The source ACL and fixture digests
matched the frozen plan, and the input source returned to its prior value.

The first text actuator failed compilation before submission. An observer
then read a pre-terminal file snapshot before its current Run was available.
The recovered observer checked the same completed sequence through the
persisted Run timestamps and original receipt. Input-source restoration
also needed an asynchronous observation. These corrections admitted zero
replacement test sequences. The four intended prompts each ran once.

## Controlled loopback

The fullscreen PTY selected the same real CLI/ACP pair, with isolated profiles
and a local provider fixture. A byte-preserving ACP relay recorded exact
prompt, RPC and session identities. At replacement provider admission the
original fixture process was already inactive. Exactly four model
generations occurred in the frozen order, with one cancelled response and
three `end_turn` responses. The next start followed the old cancelled
response. Core retained the same four distinct results.

A separate fault run held the replacement response while both follow-ups
were waiting. It delivered the byte-identical original start notification
once during that hold and once after all four Runs had settled. Both
deliveries retained the original frame digest. The active replacement,
waiting order and final idle state survived both replays. A fifth recovery
prompt then completed exactly once. The relay and provider recorded five
distinct requests and responses, and Core retained five distinct Runs.

An earlier fixture wrote its readiness file inside the workspace. Its
original Run cancelled and the remaining three Runs correctly failed the
unverified-mutation completion gate. That retained branch was inspected as a
failed outcome. The queue-only fixture writes readiness outside the
workspace, keeping the mutation gate enabled. The observer also now binds
`sendNow` to the replacement's exact prompt digest, following the actual
locally held queue rather than assuming early ACP transmission.

## Verification

Both macOS arm64 executables independently matched source fingerprint
`231f8384a785711f87cd25f0b7f172db278676d2` before these runs. The existing
paired build command was
`cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`.
The native launch used
`aster --cwd <fixture> --model deepseek/deepseek-flash --no-plan --yolo --no-subagents --fullscreen`.
The controlled cases used `--model mock/gpt` with loopback-only
configuration and disabled account imports.

[Observations](observations.json) retains the sanitized results and digests
of the private unmodified native captures and ACP traces. Final checks
confirmed absent delayed markers and inactive original processes.

## Explicitly not claimed

This covers macOS fullscreen composer send-now with two locally waiting
messages and repeated old start notifications. Reconnection, server-side
queue saturation, repeated force gestures, minimal native mode and other
operating systems remain pending. The tested diagnostic pair includes the
separate uncommitted ACP cancellation patch.
