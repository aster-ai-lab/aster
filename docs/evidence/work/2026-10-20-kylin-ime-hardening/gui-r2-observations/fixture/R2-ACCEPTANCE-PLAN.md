# R2 isolated GUI acceptance plan

No old application is launched by this preparation. `run-r2.sh` requires an explicit 64-digit `GUI_APP_COMMIT`, compares it with the installed application commit before startup, and writes it to the owned run directory. It retains all prior private mount/profile/DBus/network isolation and test-only sandbox qualifications.

1. First use a fresh profile without any gateway fixture/model configuration, matching the previous failing no-daemon diagnostic. With the separately declared passive observer, focus the rich editor, type ASCII and allow subsequent renders. Require actual target DIV/composer-tiptap/contenteditable and no React #185. Do not accept a WorkbenchHome fallback textarea or add handlers to it.
2. Start a second fresh profile with `GUI_GATEWAY_FIXTURE=1` and fcitx4.2.9.9. The only model supplier is an owned loopback HTTP server in the same network namespace. Both the synthetic member session and explicit gateway key are the fixed invalid literal `fixture-only-kylin-ime-not-an-account`. `/v1/models` advertises only `gpt-test`. Completion responses are deterministic OpenAI SSE/plain JSON with ASCII content `OFFLINE_IME_FIXTURE_OK`; no tools or file actions are returned.
3. The configured run generates a synthetic providers.json: catalog provider `openai`, enabled=true, baseUrl=<owned-loopback-origin>/v1, the fixed invalid apiKey literal, one enabled chat model `gpt-test` named 离线输入法测试模型, and defaultModel/quickModel/translateModel all selecting that pair. It also seeds `app.workspace.path` to its own private-project folder and a sentinel. These are newly generated test fixtures, not copied user provider/settings state. The native agent-plane session workspace also resides under the private profile. It does not read or copy user provider files, override send/newline keys or bypass authentication checks in product code.
4. On the actual rich surface, close IME and type `ime-plain-control`; ordinary Enter must create a visible user message, obtain the local ASCII fixture reply, and produce a recorded completion request with the control marker. This is the positive send control, not an external model test.
5. In the same actual rich composer, open fcitx and input ASCII `n i h a o`. Space must commit Chinese to the draft. Check mixed ASCII input and Escape cancellation followed by successful input. For a separate `n i h a o` preedit, Return may commit raw `nihao`; it must keep the draft and produce no new completion request. Then a normal Enter after composition must successfully submit that draft locally. Observe fresh screenshot/metadata before each decision; do not identify a surface only by appearance.
6. Repeat the same critical rich-surface checks with the official old-protocol daemon fixture, accurately retaining its archive SHA/version caveat. Corroborate visible behavior with an uninstrumented configured run; the observer is passive but may affect timing.

The model supplier records bounded request method/path/status/model/stream metadata and a hash of the last user content, never Authorization values or system prompts. It logs a boolean for the exact synthetic plain control marker. No clipboard, Unicode text injection, external account, real session/key, outside repository fixture or product file mutation is used.

Preparation validation: shell syntax and Python compilation passed. An independent private-network smoke checked model GET, streamed completion and refusal without the synthetic bearer; three requests, no application startup, no routes. See gateway-smoke.json.

New bundle command shape, after root provides and installs its commit:

```
docker exec --env GUI_APP_COMMIT=<new-commit> --env GUI_IME_DAEMON=none --env GUI_IME_TRACE=1 aster-kylin-ime-hardening bash /aster/target/evaluations/kylin-ime-gui/run-r2.sh
```

Configured mode adds `--env GUI_GATEWAY_FIXTURE=1` and uses `GUI_IME_DAEMON=modern` or `old4296`. The existing driver only uses physical XTEST ASCII keys, observed window focus and grounded clicks. `collect-proof-r2.py` captures the actual per-run commit/environment/network/profile boundaries. It never reads unrelated process environments.
