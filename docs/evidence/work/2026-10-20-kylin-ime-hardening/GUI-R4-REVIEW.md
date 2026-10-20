# Final installed R4 WebKit GUI review

**Passed within the isolated Linux GUI test environment:** ordinary Enter, real Fcitx Chinese input, candidate confirmation without accidental send, subsequent normal send, repeat protection and visible instant model replies. All final checks use the installed final package with its default asynchronous snooper enabled. No `FCITX_ENABLE_SYNC_MODE`, `FCITX_DISABLE_SNOOPER` or `IBUS_DISABLE_SNOOPER` override is present.

- App commit: `9ccb0ea27ee2a19dd63506f3d5ed79be7a88856869b74a7fe74231d2378410af`.
- Canonical application bundle SHA-256, independently recomputed: `62840dfddea793ae675627be7247307cf46b2fdcb92e3c55705cbdb16918f4e1`.
- [Structured result](gui-r4-observations/result.json), [identity receipt](gui-r4-observations/identity.json), [manifest](gui-r4-observations/manifest.json), [cleanup](gui-r4-observations/cleanup.json).

## Actual application and isolation

The tested surface is the actual installed AsterDream/WebKitGTK application under the packaged legacy Flatpak 1.6 client. Modern and old-daemon runs record real `DIV.composer-tiptap` keyboard targets, composition metadata and `view.composing`; no fallback textarea is accepted as the rich editor. There are zero recorded renderer errors in the three final runs. A third fresh run omits the observer and corroborates visible behavior.

Inputs use physical XTEST ASCII keys through the real private Fcitx daemon. There is no clipboard or Unicode value injection. Each run creates private Xvfb, DBus, network and mount namespaces, XDG directories and a mounted private application profile. Startup proofs show only loopback and no routes. The generated member session, API key and provider/model/default choices are synthetic and explicitly invalid outside the fixture; no real user state, credentials or external account is read or copied. The only model supplier is an owned loopback server in the same network namespace. Its complete OpenAI-compatible response is flushed immediately; the fixture adds no response delay to hide the history race.

The passive observer only reads bounded event metadata. It does not dispatch or cancel events or change focus, DOM, editor state or values. The uninstrumented run's environment has no `LD_PRELOAD`, and its logs have zero observer records. All selected screenshot contents were visually inspected by this reviewer. Root independently reviewed the selected control/final frames modern-05/12, old-08/13 and uninstrumented-09; root did not review all eighteen. Screenshot text is not an automatic DOM assertion.

This root/Rosetta test container requires a test-only bwrap wrapper removing seccomp arguments and `WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1`. The wrapper asserts every other semantic argument is identical. These experiments are **not** physical Kylin acceptance, CUA certification, authentication testing or sandbox/security acceptance. The original unsupported CUA certification result remains unchanged.

## Modern daemon: rich input, mixed text and repeat behavior

Run `run-7fnFml`, host Fcitx 4.2.9.9, default packaged module:

1. Closing IME and physically typing `ime-plain-control` then Return gives one ordinary rich-editor keydown with `repeat=false`, one keyup and a streamed model request with the exact control marker. The user message and full `OFFLINE_IME_FIXTURE_OK` reply are visible in [modern-05](gui-r4-observations/run-7fnFml/r4-modern-05.png).
2. Opening IME, `n i h a o Space` commits **你好** ([06](gui-r4-observations/run-7fnFml/r4-modern-06.png)). Closing IME and typing `a b c` produces **你好abc** ([07](gui-r4-observations/run-7fnFml/r4-modern-07.png)). Reopening IME, `n i Escape` retains that draft ([08](gui-r4-observations/run-7fnFml/r4-modern-08.png)); `h a o Space` then produces **你好abc好** ([09](gui-r4-observations/run-7fnFml/r4-modern-09.png)).
3. A separate `n i h a o Return` commits raw spelling to draft **你好abc好nihao** ([10](gui-r4-observations/run-7fnFml/r4-modern-10.png)). The streamed-request count stays at one, and `fcitx-remote` reports state 2 (still open). The native snooper consumes candidate Return before WebKit keydown; the trace contains composition end and a single keyup. This is a real candidate no-send test, not a claim that DOM keyCode 229 reached the frontend guard.
4. Keeping Fcitx open, the next ordinary Return submits the mixed draft once. It arrives **12.47 seconds after composition end**, with both composition flags false and `repeat=false`; this particular sequence proves eventual normal sending, not immediate double-Enter behavior. The separate fast old-daemon sequence below covers that shorter interval.
5. With IME closed, a physical Return held for 1000 ms gives a first `repeat=false` keydown and ten later `repeat=true` keydowns. The first repeat starts after 663 ms, matching the XKB 660-ms delay and 40-ms interval. Exactly one additional streamed request occurs. The existing frontend repeat guard is retained.

The final [modern-12 screenshot](gui-r4-observations/run-7fnFml/r4-modern-12.png) visibly contains all three user messages and all three complete offline replies. Wire hashes and JSON-encoded lengths exactly match `ime-plain-control`, `你好abc好nihao` and `repeat-control`; private persisted user texts also match. Earlier transition screenshots are not substituted for visible reply evidence.

## Old protocol: complete GUI chain and fast second Enter

Run `run-V6V5Mp` uses the official Fcitx **4.2.9.6 archive**, SHA-256 `d55eee3f094037b5c32f0c5397d585fdf932cbc7d2091ed6f78fd7a9ae512c47`. Its upstream build reports **4.2.9.5**. This is an isolated source build, not a Kylin-distribution binary; the fixture lacks the X11 candidate-window addon. These qualifications remain in the identity receipt.

Ordinary ASCII Return produces one non-repeat keydown, one release and a visible complete offline reply ([old-05](gui-r4-observations/run-V6V5Mp/r4-old-05.png)). Real `nihao Space` produces **你好**; `ni Escape` retains it; `hao Space` produces **你好好**. A subsequent `nihao Return` leaves **你好好nihao** in the draft with no additional streamed request and IME still state 2. Keeping IME open, ordinary Return sends that draft and visibly renders its reply ([old-11](gui-r4-observations/run-V6V5Mp/r4-old-11.png)). This old GUI run does **not** claim the modern run's `abc` mixed-language check.

A further preedit receives two physical Returns in a single bounded driver action, with no screenshot or inspection between them. The first ends composition and commits raw `nihao`. The second rich-editor keydown occurs **370 ms after composition end**, with `isComposing=false`, `view.composing=false` and `repeat=false`. The driver's command-start press gap is 411.32 ms; the measured composition-to-second-key interval is the 370-ms value. IME stays open. No completion request occurs between the two physical keydown command starts; the only new streamed request is recorded after the second. Its hash exactly matches `nihao`. The final [old-13 screenshot](gui-r4-observations/run-V6V5Mp/r4-old-13.png) shows all three messages and replies. This closes the realistic fast continue-to-send check without weakening IME or repeat guards.

## Uninstrumented corroboration

Run `run-Wfa1CS` uses a fresh profile, the modern daemon, identical instant response fixture, and no observer/preload or IME-mode override. Ordinary Return visibly sends the control message and renders the reply. `nihao Space` commits **你好**; a separate `nihao Return` leaves **你好nihao** in the draft with streamed count still one and IME state 2. Keeping IME open, the next ordinary Return sends once. Both complete replies and the Chinese/raw user message are visible in [uninstrumented-09](gui-r4-observations/run-Wfa1CS/r4-uninstrumented-09.png). The request hash and private user text match the observed draft. This is visible corroboration, not a second DOM event trace.

## Evidence and completion

The final evidence contains **3 runs, 18 selected screenshots, 724 bounded observer records, 8 streamed completion requests and zero renderer-error records**. Separate non-streamed title requests are labelled and do not count as extra user sends. Every candidate checkpoint records a streamed count of one. Per-run action logs include IM state, and startup proofs bind the same final commit, selected module/cache, default environment, private profile and loopback-only network. The manifest covers the fixture sources, observer source, structured checks and selected screenshots.

All three fixtures completed normal stop-file cleanup; application PIDs 59076, 61374 and 63458 are absent. The owned container was left intact for root's final identity-checked cleanup. No product source, frozen artifact, installed application, version, resource or git state was changed by this GUI verification.

The earlier fallback-only package, R2 default snooper-off failure and conditional sync/snooper experiments remain separately recorded in `GUI-REVIEW.md` and `GUI-R2-REVIEW.md`. Their failures and limits are not erased by this final success. Physical Kylin distribution/desktop/IME combinations remain the outstanding deployment-specific acceptance boundary.
