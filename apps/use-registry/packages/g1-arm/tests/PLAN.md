# Functional test plan: 宇树 G1 人形机器人 (`a3s/g1-arm`)

Every case below has an automated test. A case without one is not part of
this plan. The tests run at five levels, cheapest first:

| Level | What runs | Command |
| --- | --- | --- |
| Package | Node unit tests over the agent, world, planner, session, perception, vision menu, batch evaluation, and package shape | `node --test tests/*.test.mjs` |
| Browser | The view in Chromium under AsterDream's exact headers, sandbox, and handshake; the real `agent/run.mjs`; RGB-D, vision, and batch episodes | `node tests/browser.proof.mjs` |
| Load | View load time per phase, Chromium and WebKit, fresh context per run | `node tests/load.proof.mjs [runs]` |
| AsterDream native | Rust tests in `apps/desktop/src-tauri` (authority, view sessions, view-agent binding, warm box) and the renderer tests | see below |
| End to end | AsterDream's authority, catalog sync, asset route, turn route, warm `a3s-box`, and a gateway; Chromium plays the renderer | see below |

AsterDream native:

```sh
cd apps/desktop/src-tauri
rustup run 1.97.1 cargo test --lib -- aix_episodes workbench_projection view_agent authority
A3S_DESKTOP_BOX_LIVE=1 rustup run 1.97.1 cargo test --lib -- --ignored \
  the_g1_arm_agent_decides_in_a_live_a3s_box g1_arm_view_load_timing --nocapture
cd apps/desktop && npx vitest run src/renderer/services/plugins/__tests__/aixAgentProvider.test.ts \
  src/renderer/pages/agents/__tests__/aixAgentRuntime.test.ts \
  src/renderer/hooks/__tests__/useAssistantCatalogPresets.test.ts
cd apps/api && rustup run 1.97.1 cargo test --bin a3s-api -- workbench g1_arm the_plugin_page_offers
```

End to end (needs node, the Playwright install in `apps/desktop`, and `a3s-box`):

```sh
cd apps/desktop/src-tauri
# Hermetic: a fake member gateway that answers System One at 0.8.
rustup run 1.97.1 cargo test --lib g1_arm_runs_end_to_end -- --ignored --nocapture
# Live: the local gateway and a signed-in member session. The token stays in Rust.
A3S_DESKTOP_E2E_GATEWAY=http://127.0.0.1:8080 \
A3S_DESKTOP_E2E_MEMBER_SESSION="$HOME/Library/Application Support/dev.a3s.desktop/<instance>/member-session.json" \
  rustup run 1.97.1 cargo test --lib g1_arm_runs_end_to_end -- --ignored --nocapture
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
| P-02 | Manifest shape | Schema 3; edge agent `g1-arm` with `harness = "package"`; one contained view | pkg:package `the manifest declares schema 3…`; api:`each_workbench_manifest_declares_an_edge_agent_and_a_contained_view`; use-extension:`a3s_robot_workbenches_declare_the_workbench_and_skill` |
| P-03 | Vendored engine is the recorded upstream | MuJoCo 3.15.0 WebAssembly bytes restore exactly; `mujoco.js` is the recorded patch and generates no code at run time | pkg:package `the packed engine restores…`, `the shipped mujoco.js is the recorded patch…` |
| P-04 | The view and agent reach no host service | No import or URL outside the package | pkg:package `the view and agent reach no host service` |
| P-05 | Standard admission | The seeded archive passes `read_archive` + `accept_aix`, like an upload | api:`each_workbench_archive_passes_standard_admission` |
| P-06 | Seed list does not drift | The API's `include_bytes!` list equals the package directory minus `tests/`, `tools/`, `brand/` | api:`each_workbench_published_list_matches_the_package_directory` |
| P-07 | Listed as a plugin, not an agent | The plugin page offers it beside the cobot; the experts (智能体) catalog skips a package with a `ui` surface | api:`the_plugin_page_offers_g1_arm_beside_cobot_arm`, `the_catalog_stores_g1_arm_for_the_plugin_page`; ui:`lists an agent package but leaves a package with a view to the plugin catalog` |
| P-08 | Seeds stay apart and pins follow | The cobot and G1 seeds share no name, slug, package id, or directory; a retired G1 pin follows the activity-bar generation | api:`workbench_seeds_do_not_collide`, `a_retired_g1_arm_pin_follows_the_activity_bar_generation` |
| P-09 | Install through the catalog | `PUT /api/capabilities/installations/a3s-g1-arm` reaches the gateway once, AsterDream syncs the bundle, and the view appears in the catalogue | e2e `install`, `catalogue` (hermetic: installs 1, bundles ≥ 1) |

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
| S-01 | Scene parity with upstream | The G1 settles at the EmbodiedJev `G1Spec` travel pose (TCP within 0.2 mm of `[0.38165, -0.07967, 0.21521]`) | pkg:workbench `the G1 scene settles at the upstream travel pose` |
| S-01a | Vendored model parity | nq 45, nv 44, nu 37, robot mass 34.8305 kg, 22 robot colliders; collision hulls are MuJoCo's own and recompile unchanged | pkg:workbench `the vendored G1 keeps the upstream model…`; `tools/vendor-g1.mjs` refuses a hull that changes |
| S-02 | Every visual mesh is packed | Every mesh the scene draws (21) is in the G1 pack | pkg:workbench `the packed G1 carries every visual mesh it draws` |
| S-02a | IK restarts | A goal the current pose cannot reach is solved from a backup pose | pkg:workbench `inverse kinematics restarts from a backup pose…` |
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
| A-03 | One warm box per episode | Many turns, one `a3s-box run`; the box is removed on close | desktop:aix_episodes `a_warm_box_serves_many_turns_and_brokers_the_model_without_the_token`, `the_g1_arm_agent_decides_in_one_warm_box_through_the_real_launcher` |
| A-04 | The member token never enters the box | The host brokers the model call with the bearer; box arguments and environment carry none | desktop:aix_episodes (same two tests); e2e hermetic `foreignBearer: 0` |
| A-05 | Failure and reclaim | A failed turn removes the box; the reaper ends boxes whose view no longer binds the agent; a running turn and the box limit refuse without starting a box | desktop:aix_episodes `a_failed_turn_removes_the_box…`, `the_reaper_ends_boxes…`, `a_running_turn_and_the_box_limit_refuse…` |
| A-06 | Closed request shape | Only the closed turn and model body shapes pass; `model_request.api` is `chat` or `systemone` | desktop:aix_episodes `view_turns_and_model_bodies_accept_only_the_closed_shape` |
| A-07 | A view binds only its own agent | Another agent is 404; no bearer is 401; a bad body is 400; an unbound turn is 404; upgrade, owner change, and an undeclared harness refuse the binding | e2e `unbound`; desktop:authority `a_view_agent_turn_without_a_bound_view_is_not_found`; desktop:capabilities_view_agent_tests (2) |
| A-08 | Renderer turn port | One turn at a time, each answer carries its own id; failures reduce to the closed refusal set; withdrawal aborts and drops the late answer; messages per document are bounded | ui:aixAgentProvider (4) |
| A-09 | Live MicroVM | The published agent decides in a real `a3s-box` MicroVM; `a3s-box ps -a` shows none left | desktop:aix_episodes `the_g1_arm_agent_decides_in_a_live_a3s_box` (`A3S_DESKTOP_BOX_LIVE=1`) |
| A-10 | Whole chain | catalog → asset route → view → turn route → warm box → gateway → view; every turn 200 and model-decided | e2e hermetic and live |

## 6. Fallbacks

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| F-01 | Gateway refuses (401) | The agent chooses by its rule; the note names the 401 | browser proof `episodes[mode=rule]` |
| F-02 | Agent cannot run | The view chooses by the same rule, labels it `规则控制器（agent 不可用）`, and names the reason | browser proof `episodes[mode=refused]` |
| F-03 | One option | No model call | pkg:agent `the rule takes the only option…`; desktop:aix_episodes launcher test |
| F-04 | Missing observation | The agent asks for options | pkg:agent `a missing observation asks for options` |

## 7. Cameras, vision model, and batch evaluation

| ID | Case | Expected | Test |
| --- | --- | --- | --- |
| C-01 | Camera calibration | The external and wrist cameras project as MuJoCo does | pkg:perception `the camera calibrations match MuJoCo…` |
| C-02 | RGB-D from pixels only | Colour and depth recover the cube, destination, and barrier; an occluded cube is tracked for a bounded time, then perception refuses rather than read the simulator | pkg:perception `colour and depth recover…`, `an occluded cube is tracked…`, `a session decides on the observation hook…` |
| C-03 | RGB-D episodes | All three tasks reach 验证通过 on camera estimates; the final estimate is within 2 mm; two 640×480 JPEG views render | browser proof `cameraEpisodes` |
| C-04 | Fixed motor menu | 18 translations plus open, close, hold; a move whose target leaves the workspace is not offered and its target is not clipped | pkg:incremental `the menu is 18 fixed moves…` |
| C-05 | Vision evidence | Sensors and camera calibration only; no object pose and no success flag | pkg:incremental `vision planning evidence carries sensors…` |
| C-06 | The menu can finish every task | A quantized skill oracle finishes all three tasks inside the 80-step budget | pkg:incremental `the quantized skill finishes every task…` |
| C-07 | Safety filter | A step whose preview touches the table or barrier is refused and recorded with the arm unmoved; no other step replaces it, and repeating it stalls | pkg:incremental `the safety filter rejects the chosen step…` |
| C-08 | No rule fallback | A failed or unoffered vision answer stops in 执行异常 with no step taken, and the note says no rule replaced it | pkg:incremental `a failed vision decision stops the episode`; pkg:agent `a camera turn asks the vision model…`; browser proof `visionEpisodes[vision-unoffered]` |
| C-09 | Stall | Three identical steps with no pose or contact change stop in 决策停滞 | pkg:incremental `three identical steps…`; browser proof `visionEpisodes[vision]` |
| C-10 | Images by reference | Each vision turn carries two named JPEG frames ≤ 256 KiB; the box gets only their manifest; the chat body names each once as `aix-image:<name>` and the host puts the bytes in | browser proof `visionEpisodes` frames; desktop:aix_episodes `turn_images_reach_the_gateway_by_reference_and_never_enter_the_box` |
| C-11 | Image shape | At most two images, unique lower-case names, JPEG or PNG by magic bytes, ≤ 256 KiB decoded; a remote or raw `data:` URL, a reused image, or another part type is refused | desktop:aix_episodes `turn_images_accept_only_two_named_jpeg_or_png_frames`, `chat_bodies_take_turn_images_only_by_reference`; ui:aixAgentProvider image cases |
| C-12 | Vision request | Chat to `bailian/deepseek-v4.1-flash`, `response_format` JSON, `reasoning_effort` `none`; an answer counts only as one finished offered choice with short public summaries | pkg:agent `a vision request labels each view…`, `a planning answer counts only…`; desktop:aix_episodes `view_turns_and_model_bodies_accept_only_the_closed_shape` (`reasoning_effort` closed set) |
| E-01 | Batch plan | Every chosen mode, task, and seed once, in a fixed order; an empty or unknown choice is refused | pkg:evaluation `the plan runs every chosen mode…` |
| E-02 | Batch rows and summary | Success, status, cycles, model calls and latency per episode; success rate and cost per mode and task | pkg:evaluation `a batch row reports…`, `the summary gives…` |
| E-03 | Report storage and export | The largest batch (90 episodes) fits AsterDream's view state; the stored report equals the exported JSON and survives a reload | pkg:evaluation `the largest batch report fits…`; browser proof `batch` |

## 8. Performance budgets

| ID | Case | Budget | Test |
| --- | --- | --- | --- |
| L-01 | View interactive (navigation → first robot frame), warm browser, median of 5 | ≤ 1000 ms in Chromium, WebKit, and WKWebView on the GPU (macOS with `swift`) | `tests/load.proof.mjs` (fails over budget) with `tests/webview-load.swift` |
| L-02 | Host: catalogue, open, all view assets (debug build) | Reported; 19 files in tens of ms | desktop:`g1_arm_view_load_timing` (ignored probe) |
| L-03 | Warm agent turn | Reported; e2e records first-turn and later-turn median | e2e `episode.firstTurnMs`, `laterTurnMedianMs`; live box test |
| L-04 | Cold browser first frame | Reported, not budgeted: it is the GPU process starting | `tests/load.proof.mjs` `coldStart` |
| L-05 | In a running AsterDream window: press the activity-bar entry → view DOM ready → first robot frame on screen; first open after a launch and later opens | Reported, manual (needs AsterDream in front and the Accessibility and Screen Recording permissions) | `tests/desktop-open.swift` |

## Not covered by automation

- A running AsterDream window, in CI. `tests/desktop-open.swift` measures the load
  in one by hand; the end-to-end test uses AsterDream's real authority and
  Playwright Chromium as the renderer. No episode is run in AsterDream's own
  window. WebView2 (Windows) is not measured.
- GPU frame rate during an episode.
- The operator path `a3s-use-registry-tools assemble`.
