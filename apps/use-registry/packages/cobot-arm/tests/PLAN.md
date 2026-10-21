# Functional test plan: 七轴轻量化协作机器人 (`a3s/cobot-arm`)

Every case below has an automated test. A case without one is not part of
this plan. The tests run at five levels, cheapest first:

| Level | What runs | Command |
| --- | --- | --- |
| Package | Node unit tests over the agent, world, planner, session, and package shape | `node --test tests/*.test.mjs` |
| Browser | The view in Chromium under AsterDream's exact headers, sandbox, and handshake; the real `agent/run.mjs` | `node tests/browser.proof.mjs`, `node tests/libero.proof.mjs` |
| Load | View load time per phase, Chromium and WebKit, fresh context per run | `node tests/load.proof.mjs [runs]` |
| AsterDream native | Rust tests in `apps/desktop/src-tauri` (authority, view sessions, view-agent binding, warm box) and the renderer tests | see below |
| End to end | AsterDream's authority, catalog sync, asset route, turn route, warm `a3s-box`, and a gateway; Chromium plays the renderer | see below |

AsterDream native:

```sh
cd apps/desktop/src-tauri
rustup run 1.97.1 cargo test --lib -- aix_episodes workbench_projection view_agent authority
A3S_DESKTOP_BOX_LIVE=1 rustup run 1.97.1 cargo test --lib -- --ignored \
  the_cobot_arm_agent_decides_in_a_live_a3s_box cobot_arm_view_load_timing --nocapture
cd apps/desktop && npx vitest run src/renderer/services/plugins/__tests__/aixAgentProvider.test.ts \
  src/renderer/pages/agents/__tests__/aixAgentRuntime.test.ts \
  src/renderer/hooks/__tests__/useAssistantCatalogPresets.test.ts
cd apps/api && rustup run 1.97.1 cargo test --bin a3s-api -- workbench cobot_arm a_renamed_seed
```

End to end (needs node, the Playwright install in `apps/desktop`, and `a3s-box`):

```sh
cd apps/desktop/src-tauri
# Hermetic: a fake member gateway that answers System One at 0.8.
rustup run 1.97.1 cargo test --lib cobot_arm_runs_end_to_end -- --ignored --nocapture
# Live: the local gateway and a signed-in member session. The token stays in Rust.
A3S_DESKTOP_E2E_GATEWAY=http://127.0.0.1:8080 \
A3S_DESKTOP_E2E_MEMBER_SESSION="$HOME/Library/Application Support/dev.a3s.desktop/<instance>/member-session.json" \
  rustup run 1.97.1 cargo test --lib cobot_arm_runs_end_to_end -- --ignored --nocapture
```

Test names below are abbreviated: `pkg:` is `tests/<file>.test.mjs`,
`desktop:` is a Rust test in `apps/desktop/src-tauri/src`, `ui:` is a
renderer test in `apps/desktop/src/renderer`, `api:` is a test in
`apps/api/src/contexts`, and `e2e` is `tests/desktop.e2e.mjs` driven by
`capabilities_view_e2e_tests.rs`.

## 1. Package and catalog

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| P-01 | The package ships its declared files only | Exactly the published list; each file under the API's 8 MiB limit | pkg:package `the package ships exactly the declared files…` |
| P-02 | Manifest shape | Schema 3; edge agent `cobot-arm` with `harness = "package"`; one contained view | pkg:package `the manifest declares schema 3…`; api:`each_workbench_manifest_declares_an_edge_agent_and_a_contained_view` |
| P-03 | Vendored engine is the recorded upstream | MuJoCo 3.15.0 WebAssembly bytes restore exactly; `mujoco.js` is the recorded patch and generates no code at run time | pkg:package `the packed engine restores…`, `the shipped mujoco.js is the recorded patch…` |
| P-04 | The view and agent reach no host service | No import or URL outside the package | pkg:package `the view and agent reach no host service` |
| P-05 | Standard admission | The seeded archive passes `read_archive` + `accept_aix`, like an upload | api:`each_workbench_archive_passes_standard_admission` |
| P-06 | Seed list does not drift | The API's `include_bytes!` list equals the package directory minus `tests/`, `tools/`, `brand/` | api:`each_workbench_published_list_matches_the_package_directory` |
| P-07 | Listed as a plugin, not an agent | The plugin page offers it; the experts (智能体) catalog skips a package with a `ui` surface | api:`the_plugin_page_offers_cobot_arm`, `the_catalog_stores_cobot_arm_for_the_plugin_page`; ui:`lists an agent package but leaves a package with a view to the plugin catalog` |
| P-08 | Rename from `embodied-jev` | Members of the old seed end with exactly the current release; the old row is gone; a rerun changes nothing (in memory and PostgreSQL) | api:`a_renamed_seed_moves_its_members_in_memory`, `…_in_postgres`, `a_retired_cobot_arm_pin_follows_the_activity_bar_generation` |
| P-09 | Install through the catalog | `PUT /api/capabilities/installations/a3s-cobot-arm` reaches the gateway once, AsterDream syncs the bundle, and the view appears in the catalogue | e2e `install`, `catalogue` (hermetic: installs 1, bundles ≥ 1) |

## 2. View load and view policy

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| V-01 | AsterDream's view policy | CSP allows `'wasm-unsafe-eval'` only; `connect-src 'none'`; no `'unsafe-eval'` | desktop:authority `a_view_compiles_wasm_but_reaches_nothing`; e2e asset header checks |
| V-02 | The view loads under that policy in a sandboxed frame | AIX handshake completes; no console error or policy violation | browser proof `problems: []`; e2e `problems: []` |
| V-03 | Fonts and icons are served from the package | Three Ubuntu weights load | browser proof `fonts` |
| V-04 | Asset sessions read only the owned surface | Foreign token, agent source, the manifest, and `%2e%2e` traversal are 404 | e2e `refused`; desktop:workbench_projection `asset_sessions_only_read_the_owned_surface…` |
| V-05 | Sessions revoke | Upgrade, uninstall, owner change, close, and expiry end the session; after close the asset and turn routes are 404 | desktop:workbench_projection (20 tests); e2e `afterClose` |
| V-06 | Tampering is refused | A changed file on disk is refused and drops the verified cache | desktop:workbench_projection tamper test; reconcile `reading_every_file_verifies_the_surface_once_and_refuses_tampering` |
| V-07 | Centered loader | AsterDream's shared loader covers the view until its document loads | ui:`covers a loading view with the shared centered loader until its document loads` |

## 3. Simulation and tasks

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| S-01 | Scene parity with upstream | The Panda settles at the upstream travel pose (TCP `[0.37997, -0.07999, 0.22001]`) | pkg:workbench `the Panda scene settles at the upstream travel pose` |
| S-02 | Every visual mesh is packed | Every mesh the scene draws is in the Panda pack | pkg:workbench `the packed Panda carries every visual mesh it draws` |
| S-03 | Three tasks | 搬运入盘, 方块堆叠, 越障搬运 each reach 验证通过 in 8 cycles under the rule baseline | pkg:workbench `the rule baseline completes all three tasks…` |
| S-04 | Seeds | A seed moves the source cube | pkg:workbench `seeds move the source cube` |
| S-05 | Speed | An unpaced episode runs faster than real time, previews included | pkg:workbench `an unpaced episode runs faster than real time…` |
| S-06 | History and replay | History and 轨迹回放 render after an episode | browser proof `historyHeading`, `replayLabel` |

## 4. Decision model and threshold

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| D-01 | Offered choices only | Every decision offers its baseline; a choice outside the offer stops the episode | pkg:workbench `every decision offers its baseline…` |
| D-02 | The decision model by default | One System One `choice` question over exactly the offered keys, to `platform/apofasi`, with no credential | pkg:agent `the decision request asks one choice question…`, `the model id is…`, `the model request uses only fields the host forwards…`; e2e hermetic models = `{platform/apofasi}` |
| D-03 | A decision counts only when well formed | The choice is offered, the distribution covers exactly the offered keys and is normalized | pkg:agent `a decision counts only with an offered choice…`, `a model reply counts only when it names one offered key` |
| D-04 | Threshold value and control | The threshold reads `0.55` and is enabled when idle | browser proof `thresholdLabel`, `thresholdEnabled` |
| D-05 | Below the threshold waits for a person | A chosen probability under the threshold stops in 等待人工处理 with no step taken; 继续实验 goes on | pkg:workbench `a choice below the threshold waits for a person…`; browser proof `unsure`; e2e check `outcome ⇔ p < threshold` (live: p 0.4225 < 0.55 → 等待人工处理) |
| D-06 | At or above the threshold proceeds | The 0.8 episode reaches 验证通过 in 8 model-decided steps | browser proof `episodes[mode=model]`; e2e hermetic |

## 5. Agent, box, and gateway

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| A-01 | Warm agent protocol | One process serves many runs with one process-wide sequence; every reply is one `Content-Length` JSON body | pkg:agent `the warm entry serves many runs…`, `every reply is one Content-Length JSON body…` |
| A-02 | Malformed traffic | Malformed, duplicate, and unknown traffic is refused | pkg:agent `the entry refuses malformed, duplicate, and unknown traffic` |
| A-03 | One warm box per episode | Many turns, one `a3s-box run`; the box is removed on close | desktop:aix_episodes `a_warm_box_serves_many_turns_and_brokers_the_model_without_the_token`, `the_cobot_arm_agent_decides_in_one_warm_box_through_the_real_launcher` |
| A-04 | The member token never enters the box | The host brokers the model call with the bearer; box arguments and environment carry none | desktop:aix_episodes (same two tests); e2e hermetic `foreignBearer: 0` |
| A-05 | Failure and reclaim | A failed turn removes the box; the reaper ends boxes whose view no longer binds the agent; a running turn and the box limit refuse without starting a box | desktop:aix_episodes `a_failed_turn_removes_the_box…`, `the_reaper_ends_boxes…`, `a_running_turn_and_the_box_limit_refuse…` |
| A-06 | Closed request shape | Only the closed turn and model body shapes pass; `model_request.api` is `chat` or `systemone` | desktop:aix_episodes `view_turns_and_model_bodies_accept_only_the_closed_shape` |
| A-07 | A view binds only its own agent | Another agent is 404; no bearer is 401; a bad body is 400; an unbound turn is 404; upgrade, owner change, and an undeclared harness refuse the binding | e2e `unbound`; desktop:authority `a_view_agent_turn_without_a_bound_view_is_not_found`; desktop:capabilities_view_agent_tests (2) |
| A-08 | Renderer turn port | One turn at a time, each answer carries its own id; failures reduce to the closed refusal set; withdrawal aborts and drops the late answer; messages per document are bounded | ui:aixAgentProvider (4) |
| A-09 | Live MicroVM | The published agent decides in a real `a3s-box` MicroVM; `a3s-box ps -a` shows none left | desktop:aix_episodes `the_cobot_arm_agent_decides_in_a_live_a3s_box` (`A3S_DESKTOP_BOX_LIVE=1`) |
| A-10 | Whole chain | catalog → asset route → view → turn route → warm box → gateway → view; every turn 200 and model-decided | e2e hermetic and live |

## 6. Fallbacks

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| F-01 | Gateway refuses (401) | The agent chooses by its rule; the note names the 401 | browser proof `episodes[mode=rule]` |
| F-02 | Agent cannot run | The view chooses by the same rule, labels it `规则控制器（agent 不可用）`, and names the reason | browser proof `episodes[mode=refused]` |
| F-03 | One option | No model call | pkg:agent `the rule takes the only option…`; desktop:aix_episodes launcher test |
| F-04 | Missing observation | The agent asks for options | pkg:agent `a missing observation asks for options` |

## 7. LIBERO

`libero proof` is `tests/libero.proof.mjs`.

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| B-01 | Physics parity with robosuite | Replaying robosuite's expert: the drawer TCP within 1 mm and success at the same step (96); the microwave exact (< 0.1 mm) before contact and success within 10 steps of robosuite | pkg:libero `the drawer replay tracks robosuite…`, `the microwave replay is exact before contact…` |
| B-02 | Camera calibration and init states | Live intrinsics and the agentview extrinsics match robosuite within 1e-6, the wrist within 1e-4; TCP and hand quaternion match the observation; each of the 10 exported init states is unsolved, puts the TCP within 0.1 mm of robosuite's, and keeps the collision hulls within 0.1 mm; an init outside 0–9 or not an integer is refused | pkg:libero `the live cameras reproduce robosuite calibration…`, `every exported init state starts unsolved…`; libero proof `initNine` |
| B-03 | Bounded memory | Each scene reserves at most 16 MiB of constraint arena | pkg:libero `each LIBERO model reserves a bounded constraint arena` |
| B-04 | Render fidelity | The first external and wrist images are within 15 of robosuite's own render per 16 px block and 10 in mean colour (of 255) | libero proof `scenes.*.fidelity` |
| B-05 | Grounding | Projection and back-projection invert; a plan moves only when it matches the waypoint schema and bounds exactly; a pixel target is back-projected through rendered depth | pkg:libero `back-projection and projection invert…`, `a plan is grounded only when…`, `the reference planner names a calibrated pixel…` |
| B-06 | Local controller | Each axis moves toward the waypoint with upstream's magnitude rule | pkg:libero `the local controller moves each axis…` |
| B-07 | Reference baseline | The privileged planner closes both tasks from every init state within the budget (drawer 200 steps, microwave 500) with no model call | pkg:libero `the privileged reference planner closes both LIBERO tasks from every init state…`; libero proof `batch` |
| B-08 | Vision request | Only a LIBERO protocol turn plans; images go to a chat model, never the decision model; at most two distinct safe images; the request is the upstream prompt with `aix-image:` references and no image bytes | pkg:agent `a LIBERO turn is recognised only by its protocol…`, `images go to a chat model…`, `the image manifest names…`, `the LIBERO request is the upstream planner prompt…` |
| B-09 | Vision reply | A reply counts only as one finished message with exactly the plan fields | pkg:agent `a LIBERO reply counts only as one finished message…` |
| B-10 | No substitute plan | Missing or wrong images, a malformed plan, or a 401 stop the episode with no motion and no rule plan | pkg:agent `the warm entry plans a LIBERO waypoint only from both images…`; pkg:libero `an invalid plan or an unavailable model stops the episode…`; libero proof `scenes.*.refusal` |
| B-11 | Whole vision chain | Capture → agent → gateway → plan → grounding → OSC → `Close` predicate closes the drawer; the history shows the plan's own camera input | libero proof `vision` |
| B-12 | Budgets | A planner that never progresses ends `exhausted` at 60 plans and is not a success | pkg:libero `a planner that never progresses exhausts the plan budget…` |
| B-13 | Batch evaluation | Run i starts from init state i; rows and summary for 2 planners × 2 tasks × 10 runs fit the view state; 11 runs are refused; the report is stored | pkg:libero `the batch report keeps every planner, task and run…`; libero proof `batch.rows[].init`, `batch.stored` |

## 8. Performance budgets

| ID | Case | Budget | Test |
| --- | --- | --- | --- |
| L-01 | View interactive (navigation → first robot frame), warm browser, median of 5 | ≤ 1000 ms in Chromium, WebKit, and WKWebView on the GPU (macOS with `swift`) | `tests/load.proof.mjs` (fails over budget) with `tests/webview-load.swift` |
| L-02 | Host: catalogue, open, all view assets (debug build) | Reported; 18 files in tens of ms | desktop:`cobot_arm_view_load_timing` (ignored probe) |
| L-03 | Warm agent turn | Reported; e2e records first-turn and later-turn median | e2e `episode.firstTurnMs`, `laterTurnMedianMs`; live box test |
| L-04 | Cold browser first frame | Reported, not budgeted: it is the GPU process starting | `tests/load.proof.mjs` `coldStart` |
| L-05 | In a running AsterDream window: press the activity-bar entry → view DOM ready → first robot frame on screen; first open after a launch and later opens | Reported, manual (needs AsterDream in front and the Accessibility and Screen Recording permissions) | `tests/desktop-open.swift` |

## Not covered by automation

- A running AsterDream window, in CI. `tests/desktop-open.swift` measures the load
  in one by hand; the end-to-end test uses AsterDream's real authority and
  Playwright Chromium as the renderer. No episode is run in AsterDream's own
  window. WebView2 (Windows) is not measured.
- GPU frame rate during an episode.
- A LIBERO episode against the real vision model, as a repeatable test. The
  proofs use a stand-in gateway that answers scripted waypoints, so they prove
  the chain, not the model's success rate. Real-model batches (0/2 from init
  state 0; drawer 0/3 and microwave 1/2 completed from init states 0–2) are
  retained as evidence in
  `docs/evidence/desktop-plugins/20261012-cobot-arm-libero/`.
- The operator path `a3s-use-registry-tools assemble`.
