# 七轴轻量化协作机器人

Package `a3s/cobot-arm@1.0.0`. One self-contained `.aix` package, three surfaces:

| Surface | Id | Role |
| --- | --- | --- |
| Skill | `cobot-arm` | Reads one observation and returns exactly one offered `choice`. |
| Agent | `cobot-arm` | Edge package harness, entry `agent/run.mjs`, 500 cpu millis, 256 MiB. AsterDream keeps one warm `a3s-box` per open view. |
| UI | `workbench` | Mini app (`studio.applets`): no Activity Bar entry; after install, 打开 opens it in its own system window. Root `ui/workbench`. |

Nothing runs on the host outside AsterDream. There is no lab server, no port, and no Python.

## Workbench

The view is the EmbodiedJev 虚拟实验室 workbench, ported into the package: the same layout, styles, three.js scene, and controls. Its `/api` calls are answered inside the page, so there is no lab server. Multi-model comparison and connection settings are not part of the package. LIBERO evaluation is, on two libero_90 tasks (see LIBERO below).

- `world.js` builds the Franka Panda (MuJoCo Menagerie, packed in `vendor/panda/panda.js`) with a table and one of three task scenes: 搬运入盘 (red cube into the blue tray), 方块堆叠 (red cube onto the blue cube), and 越障搬运 (over a barrier into the tray). Physics is the official MuJoCo 3.15.0 WebAssembly build in `vendor/mujoco/`. The arm uses position servos and damped least-squares inverse kinematics; the grasp is finger friction.
- `planning.js` offers the phases that apply to the current state and previews each candidate action on a copy of the world. A candidate that touches the table or barrier, loses the grasp, or drops the cube is refused.
- `session.js` runs one episode: choose a phase, preview candidates, choose an action, execute it. An episode is verified when the physical success condition holds. It stops after 30 actions or three repeated actions without effect.
- `robot-scene.js` and `app.js` draw the scene with three.js and run the page.

The rule baseline completes each task in 8 cycles, the same as upstream `run_headless`.

## Decisions

Each cycle has two decisions: a phase, then an action. When only one key is offered, the page takes it without a call. Otherwise it sends the observation to the package agent with `aix.agent.invoke`. AsterDream runs `agent/run.mjs` in a warm edge box for the life of the view:

1. The agent emits a `model_request`. For the default decision model `platform/apofasi`, the request has `api: "systemone"` and a System One body: one `choice` question whose criteria are exactly the offered keys. For any other model given in `parameters.model`, it is a chat-completion body. AsterDream checks the body, calls the a3s gateway (`/v1/systemone` or `/v1/chat/completions`) with the member session, and posts the reply to `/v1/agent/model-responses`. The box never holds a credential.
2. A System One reply counts only when it chooses an offered key and publishes a probability for exactly the offered keys, summing to 1. A chat reply counts only when it names one offered option. Otherwise, or when the gateway is unavailable, the agent uses the rule: the only option, else an offered `baseline_choice`, else the first option.
3. The output is `{"choice", "source": "model" | "rule", ...}`, plus `probabilities`, `confidence`, and the serving `model` from the decision model. The page refuses a choice it did not offer.

决策门槛 (default 0.55) works as upstream. When the chosen key's probability is below it, the episode stops in 等待人工处理 until the user continues; lower the threshold to let it go on. A rule decision has no probability and never stops. On the lexical decision model in this repo's gateway, the chosen probability was 0.42–0.79 on the three tasks, so at 0.55 most action steps wait for a person.

When the agent cannot run (for example, `a3s-box` is not installed), the page chooses with the same rule and labels the step `规则控制器（agent 不可用）` with the reason.

## LIBERO

The LIBERO 基准 section runs two libero_90 tasks (LIBERO revision `8f1084e3`, robosuite 1.4.1) in MuJoCo WASM: task 0 关闭顶层抽屉 and task 33 关闭微波炉. Each scene carries LIBERO's first 10 init states of its task (0–9), each settled as robosuite settles it; 初始状态 chooses one. The init states move the objects (the microwave door opens to between −1.34 and −1.98 rad) and leave the arm's start pose the same. Success is LIBERO's `Close` predicate on the exported joint thresholds.

- `vendor/libero/{drawer,microwave}.js` carry each scene: the physics MJCF with every mesh inline, the settled init states, the OSC_POSE controller parameters, the camera calibration, and a render pack. The render pack holds the geoms robosuite's cameras draw (groups 1–2), with the colour and finish MuJoCo resolves for each. The meshes are simplified on the welded surface, each keeping its area within 3% of the source, so the simplifier neither tears holes nor fills real ones such as the microwave door's window.
- `libero-world.js` steps the scene with robosuite's OSC_POSE and Panda gripper at 20 Hz. Replaying robosuite's own expert actions, the drawer TCP stays within 1 mm and succeeds on the same step (96). The microwave matches within 0.1 mm for 80 steps and succeeds within 10 steps of robosuite (313 vs 310).
- `libero-scene.js` renders the agentview (外部) and eye-in-hand (腕部) cameras as 384 px RGB-D. It lights them like MuJoCo's fixed pipeline: a headlight, the model's spot lights, and specular folded into the lit colour before the texture modulates it. Against robosuite's own images of the same state, the 16 px block error is 4–9 of 255. MuJoCo's shadows are not drawn.
- `libero-session.js` follows EmbodiedJev's `libero-rgbd-waypoint-v1`: each plan names a world, relative, or pixel waypoint. A pixel target is back-projected through the rendered depth. The local controller moves toward the waypoint for up to 600 steps and 60 plans.

Two planners:

- 视觉模型 sends the two annotated JPEGs (a 64 px grid and a cyan cross at the projected TCP) and the robot's own state to the package agent. The agent asks the vision chat model (`bailian/deepseek-v4.1-flash`, or the chat model in `parameters.model`) through the gateway. The images go as `aix-image:` references that AsterDream fills in; the agent never sees the bytes. A malformed plan, an image set that is not exactly `external` + `wrist`, or an unavailable model stops the episode before any motion. There is no rule fallback.
- 参考航点 is a privileged baseline: waypoints from the simulator's internal state. It pushes the microwave door at a point fixed in the door's frame, so it follows each init state's opening angle. From the 10 init states it closes the drawer in 156–157 steps and the microwave in 316–446, and makes no model call.

批量评测 runs the chosen planners on the chosen tasks, 1 to 10 runs each. Run i starts from init state i, as LIBERO's own evaluation does. Per planner and task it reports the success rate, steps, plans, model calls, model latency, and wall time. It stores the report in the view state, and 导出报告 shows it as JSON to copy.

<a id="studio-view-policy"></a>

## AsterDream view policy

The view document is sandboxed (`allow-scripts`, opaque origin) with `connect-src 'none'` and no `'unsafe-eval'`. The page fetches nothing; scripts and fonts load as view files:

- `vendor/mujoco/wasm.js` carries the gzip-compressed WebAssembly as base64. `engine.js` inflates it in memory, and the policy's `'wasm-unsafe-eval'` lets the page compile it.
- `vendor/mujoco/mujoco.js` is patched so embind builds its invokers as closures, not with `new Function`.
- `vendor/panda/panda.js` carries the Panda model and meshes the same way.
- `vendor/libero/{drawer,microwave}.js` carry the LIBERO scenes; each is loaded only when its task is chosen.

The vendor tools regenerate these files: `tools/vendor-mujoco.mjs` (MuJoCo; refuses an upstream file the patch does not match), `tools/vendor-panda.mjs` (Panda), and `tools/vendor-viewer.mjs` (three.js, Lucide icons, Ubuntu fonts). `tools/libero/` exports the LIBERO scenes from a LIBERO checkout on robosuite (Python). The order and environment are in the docstring of `tools/libero/common.py`. `validate.py` and `js-parity.mjs` replay robosuite's expert against the export, and `render_probe.py` measures MuJoCo's lighting.

## Tests

```sh
cd apps/use-registry/packages/cobot-arm
node --test tests/*.test.mjs
node tests/browser.proof.mjs
node tests/libero.proof.mjs
node tests/load.proof.mjs
```

The unit tests cover the agent protocol, the world, planner, and session, the LIBERO replay, calibration, grounding, and budgets, and the package shape.

The LIBERO proof runs the real agent in Chromium with a stand-in gateway. It compares each scene's first camera images with robosuite's (`tests/fixtures/libero/`). It then closes the drawer through the vision path, checks that a 401 stops with no motion, runs the reference batch on both tasks from init states 0 and 1, and loads the microwave's init state 9. The browser proof loads the view in Chromium under AsterDream's exact view policy and sandbox, and drives the real `agent/run.mjs` through the host turn loop. It runs four episodes: System One replies at 0.8, a 401 gateway (the agent uses its rule), a refused agent turn (the page uses its rule and labels it), and System One replies at 0.4, which must stop in 等待人工处理. It needs the Playwright install in `apps/studio`.

The load proof measures navigation to the first robot frame, phase by phase, in Chromium, in WebKit when Playwright has it, and on macOS with `swift` in WKWebView on the GPU (`tests/webview-load.swift`), the engine of AsterDream's window. It fails when a median exceeds 1000 ms (`LOAD_BUDGET_MS`). `LOAD_VIEW_ROOT` measures another copy of the package, for a before/after comparison.

`tests/studio-open.swift` measures the same load in a running AsterDream window, by hand: it opens the installed workbench from the activity bar and times the press to the first robot frame on screen.

The end-to-end test runs AsterDream's real authority: catalog install, view session, asset route, view-agent turn route, a warm `a3s-box`, and the member gateway. `tests/studio.e2e.mjs` plays the renderer in Chromium. It is the ignored Rust test `cobot_arm_runs_end_to_end_through_studio` in `apps/studio/src-tauri`. Without environment it serves a fake gateway; with `A3S_STUDIO_E2E_GATEWAY` and `A3S_STUDIO_E2E_MEMBER_SESSION` it runs against a real one, and the member token stays in Rust.

`tests/PLAN.md` is the functional test plan. It maps each case to the test that proves it.

`tests/`, `tools/`, and `brand/` are package source only. The API publishes an explicit file list (`apps/api/src/contexts/plugin/infrastructure/cobot_arm_package.rs`) that excludes them; `each_workbench_published_list_matches_the_package_directory` fails when this directory and the list differ.

## Supply

This directory is package source. The admission is `a3s/cobot-arm` in `admissions.acl`. Operators publish it with `a3s-use-registry-tools assemble`. The API seeds the same files as the catalog package `a3s-cobot-arm`.

Third-party notices: `ui/workbench/THIRD_PARTY_NOTICES.md`.
