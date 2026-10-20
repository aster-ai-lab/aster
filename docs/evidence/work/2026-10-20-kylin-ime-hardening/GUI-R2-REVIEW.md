# R2 installed WebKit input review

**Result: the R11 render loop is fixed, but this installed R2 package fails ordinary Enter with its default asynchronous Fcitx module.** Restoring the upstream key snooper in an app-only environment A/B resolves the repeat misclassification while retaining asynchronous filtering. A newly built package with that default still requires its own final run.

Installed commit: `d588d6d5e186e0b960198af3dd75956ea4cd890367e47d4dcccf519b5a369bc8`. The earlier d847 package and fallback-only observations remain separately recorded in `GUI-REVIEW.md`; they are not upgraded by these results.

## Method and limits

These are the actual installed AsterDream, WebKitGTK and rich `DIV.composer-tiptap` editor, driven through XTEST ASCII key presses into private Xvfb. A separately declared LD_PRELOAD observer records passive event metadata only. It neither reads draft values nor changes events, focus, DOM or editor state. Screenshots were visually inspected; their text is not an automatic DOM assertion. The observer can affect timing, so final default-package confirmation should also include an uninstrumented run.

Every run has a new mount namespace, loopback-only network namespace, DBus session, XDG roots and mounted private Flatpak profile. All routes are empty. No user state is copied. The configured fixture generates an explicitly invalid synthetic member session, an `openai` provider with a loopback URL, fixed invalid API key and single `gpt-test` model, three default-model choices, and a workspace ACL pointing to its own test project. It does not override send/newline preferences. The same network namespace contains a deterministic local model server; no external account or chat service is used. The server stores bounded request metadata, content hashes and a control-marker boolean, not authorization values or system prompts.

The test runs the actual legacy Flatpak 1.6 executable. This root/Rosetta container needs a test-only bwrap wrapper removing seccomp arguments and `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1`. These qualifications are retained in startup proof and fixture sources. This is not physical Kylin acceptance, authentication acceptance, CUA certification or sandbox acceptance. AT-SPI exposes the GTK frame but not a usable WebKit DOM.

## Controlled results

| Run | Only relevant condition | Observed result |
| --- | --- | --- |
| `run-O2FBdB` | Fresh no model/no daemon, matching previous crash configuration | Actual rich editor survives focus, ASCII input and later renders; no renderer error. Enter reports no available model and retains draft. R11 regression fixed. |
| `run-wCZiI4` | Configured model, Fcitx 4.2.9.9, default async/snooper off | All 18 observed keydown captures, including the first ordinary Enter, have `repeat=true`. Each release is delivered twice. Enter is correctly prevented by the existing repeat guard, but no completion POST occurs. |
| `run-o97yDH` | Same model/daemon, only `FCITX_ENABLE_SYNC_MODE=1` | Ordinary Enter has `repeat=false`, one release and submits exactly one streamed request. This is a diagnostic environment, not the selected product fix. |
| `run-dTuzP4` | Same model/daemon, async retained, only `FCITX_DISABLE_SNOOPER=0` and `IBUS_DISABLE_SNOOPER=0` | Ordinary Enter has `repeat=false`, one release and submits. Chinese Space, mixed input, Escape cancellation and candidate Return behave correctly; details below. |
| `run-t8QUV5` | Default async/snooper off; private X server autorepeat disabled | First ordinary Enter still has `repeat=true`, duplicate releases and no completion POST. Genuine X autorepeat is excluded. Autorepeat restored before cleanup. |
| `run-vzqGQ5` | Same configured model, no IM daemon | Ordinary Enter has `repeat=false`, one release and one streamed request. User message, cleared draft and `OFFLINE_IME_FIXTURE_OK` reply are visibly rendered. |

The X server reports autorepeat delay **660 ms**, interval **40 ms**. Ordinary XTEST taps last **61 ms** between DOM press and release. In the default failing run with server autorepeat off, Enter capture seq214 is `repeat=true`; seq216 and seq218 are two separate window-capture releases with the same timestamp. In the snooper-on run, seq180 is `repeat=false` and seq182 is the only release. Both rich-editor composition flags and all modifiers are false. The native forwarding/replay source analysis is maintained separately by the delivery reviewer.

These controls identify the Fcitx asynchronous widget-filter/replay route as the cause; they do not justify weakening the frontend repeat guard or imposing blocking synchronous DBus calls as the product default.

## Actual IME sequence with asynchronous snooper enabled

`run-dTuzP4` used the already installed R2 binary and app-only environment override, not a rebuilt module. All inputs used physical ASCII keys and the real private Fcitx daemon.

1. Closing IME, typing `ime-plain-control` and pressing Return creates a visible user message, clears the rich draft and records one streamed model request with the exact control marker.
2. Opening IME and pressing `n i h a o Space` produces draft **你好** (`r2-snooper-07.png`). Switching to English and typing `a b c` produces **你好abc** (`08`).
3. Opening IME, pressing `n i Escape` leaves **你好abc** (`09`). Then `h a o Space` produces **你好abc好** (`10`). The fresh valid screenshot is used; no black frame is counted as evidence.
4. A separate `n i h a o Return` commits the raw spelling into the draft, **你好abc好nihao** (`11`), with no new streamed request. Here the native snooper consumes candidate Return before WebKit keydown; metadata shows composition end and one keyup. This proves the real candidate action does not submit; it does not claim that this sequence reached the frontend 229 guard.
5. Closing IME and pressing a new ordinary Return submits that draft: streamed-request count changes from one to two. Its request hash and byte count match the synthetic mixed draft; the clear draft and user message provide the UI send control.
6. Typing `repeat-control`, then physically holding Return for 1000 ms yields a first `repeat=false` keydown, nine later `repeat=true` keydowns starting after 662 ms, and one release. Exactly one additional streamed request occurs. Existing frontend repeat protection remains effective.

Snooper and sync screenshots show a visible user message and draft reset; the captured assistant area is blank even though the deterministic reply exists in private persisted messages. Those runs are **not** claimed as visible assistant-reply UI acceptance. The no-daemon run has a separate valid screenshot showing the reply. No product change was made to hide or broaden this fixture/UI limitation.

## Reproducibility and remaining scope

`gui-r2-observations/` contains six runs, thirteen selected screenshots, full bounded passive event records, zero renderer-error records, actions, model-request metadata, startup/network/profile proofs, XKB controls, fixture sources, machine-readable validation and SHA-256 manifest. `R2-ACCEPTANCE-PLAN.md` in the fixture snapshot records the generated provider fields and read/write boundaries. Two earlier gateway-only/wrong-provider configuration attempts are retained under target as fixture corrections and are not IME failures or passes.

All six application runs exited through their owned stop-file cleanup. A final package restoring the upstream snooper default must be tested without these environment overrides, using both the modern daemon and the official old-protocol fixture, plus an uninstrumented corroboration. The official old fixture's archive/version caveat must remain explicit; it is not a Kylin-distribution binary.
