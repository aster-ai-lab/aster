# 宇树 G1 人形机器人

Package `a3s/g1-arm@1.0.0`. One self-contained `.aix` package, three surfaces:

| Surface | Id | Role |
| --- | --- | --- |
| Skill | `g1-arm` | Reads one observation and returns exactly one offered `choice`. |
| Agent | `g1-arm` | Edge package harness, entry `agent/run.mjs`, 500 cpu millis, 256 MiB. AsterDream keeps one warm `a3s-box` per open view. |
| UI | `workbench` | Mini app (`desktop.applets`): no Activity Bar entry; after install, 打开 opens it in its own system window. Root `ui/workbench`. |

Nothing runs on the host outside AsterDream. There is no lab server, no port, and no Python.

The package is the EmbodiedJev G1 integration (`apps/desktop/g1.zip`) ported onto the same workbench as `a3s/cobot-arm`: the same layout, controls, tasks, planner, session, agent, and view policy. The robot is different.

## Workbench

- `world.js` builds the G1 station of EmbodiedJev `G1Spec`: a Unitree G1 (MuJoCo Menagerie `g1_with_hands`, packed in `vendor/g1/g1.js`) with its pelvis welded at the table edge, the torso pitched 0.25 rad forward, and a Franka Panda parallel gripper on the right wrist. Only the torso, head, and right arm are drawn and collide; the legs, left arm, and hands keep their inertia. The right arm and waist pitch are position servos (kp 1500) with gravity compensation. Inverse kinematics is damped least squares that restarts from backup poses when the current pose gets stuck. The grasp is finger friction.
- The three task scenes are the same as the cobot: 搬运入盘 (red cube into the blue tray), 方块堆叠 (red cube onto the blue cube), and 越障搬运 (over a barrier into the tray). Physics is the official MuJoCo 3.15.0 WebAssembly build in `vendor/mujoco/`.
- `planning.js` offers the phases that apply to the current state and previews each candidate action on a copy of the world. A candidate that brings the arm or hand against the table or barrier, loses the grasp, or drops the cube is refused.
- `session.js` runs one episode: choose a phase, preview candidates, choose an action, execute it. An episode is verified when the physical success condition holds. It stops after 30 actions or three repeated actions without effect.
- `robot-scene.js`, `meshes.js`, and `app.js` draw the scene with three.js and run the page.

The rule baseline completes each task in 8 cycles.

## Observation modes

The 观测方式 switch chooses what a decision sees. The cameras are EmbodiedJev's: an external view and a wrist view, each 640×480, rendered by three.js from the same scene (`RobotScene.sensorRig`).

- **仿真真值**: the simulator's object and destination poses, as upstream's privileged mode.
- **RGB-D 相机**: `perception.js` ports upstream `perception.py`. It segments the cube, destination, and barrier by colour, back-projects their depth with each camera's calibration, and fuses the two views. A hidden object is tracked for a bounded time; after that perception refuses and the episode stops. The simulator pose is never used to fill a gap. The panel shows the estimate error against the simulator; the agent never sees it.
- **视觉模型**: `incremental.js` ports upstream `incremental.py`. Each decision offers 21 fixed motor commands: translations of 40, 10, or 2 mm along ±x, ±y, ±z, and open, close, hold. A move whose target leaves the workspace is not offered; its target is never clipped to fit. The planning state has sensors, camera calibration, goal conditions, and the last six outcomes, with no object pose and no success flag. Both camera images go with the turn. A simulator preview of the chosen step refuses it if the arm would touch the table or barrier; the refusal is recorded for the next decision and no other step replaces it. There is no rule fallback: a failed or unoffered answer stops the episode in 执行异常. Three identical steps with no change stop it in 决策停滞, and the budget is 80 steps, inside which a scripted skill on the same menu finishes every task.

## Decisions

Each cycle has two decisions: a phase, then an action. When only one key is offered, the page takes it without a call. Otherwise it sends the observation to the package agent with `aix.agent.invoke`. AsterDream runs `agent/run.mjs` in a warm edge box for the life of the view:

1. The agent emits a `model_request`. For the default decision model `platform/apofasi`, the request has `api: "systemone"` and a System One body: one `choice` question whose criteria are exactly the offered keys. For any other model given in `parameters.model`, it is a chat-completion body. AsterDream checks the body, calls the a3s gateway (`/v1/systemone` or `/v1/chat/completions`) with the member session, and posts the reply to `/v1/agent/model-responses`. The box never holds a credential.
2. A System One reply counts only when it chooses an offered key and publishes a probability for exactly the offered keys, summing to 1. A chat reply counts only when it names one offered option. Otherwise, or when the gateway is unavailable, the agent uses the rule: the only option, else an offered `baseline_choice`, else the first option.
3. The output is `{"choice", "source": "model" | "rule", ...}`, plus `probabilities`, `confidence`, and the serving `model` from the decision model. The page refuses a choice it did not offer.

决策门槛 (default 0.55): when the chosen key's probability is below it, the episode stops in 等待人工处理 until the user continues. A rule decision has no probability and never stops.

When the agent cannot run (for example, `a3s-box` is not installed), the page chooses with the same rule and labels the step `规则控制器（agent 不可用）` with the reason.

A vision decision is different. The page sends the two camera frames with the turn as named JPEG data URLs (`external`, `wrist`, each ≤ 256 KiB). AsterDream checks them and gives the agent only a manifest (`name`, `media_type`, `bytes`). The agent's chat request to `bailian/deepseek-v4.1-flash` labels each view and names it as `aix-image:<name>`; AsterDream puts the bytes in before calling the gateway, so the box never holds an image or a credential. The request sets `reasoning_effort: "none"`: with hidden reasoning on, this model spent all 4096 output tokens per step and answered nothing. An answer counts only as one finished JSON object with an offered `choice` and optional short public `intent` and `visual_evidence`, which the panel shows.

## Batch evaluation

批量评测 runs the chosen observation modes × tasks × seeds (1–10 per pair) in order, with shorter animation pauses than a single run, and reports, per mode and task, the success count, mean steps, model calls, and mean model latency. Each row keeps the fields of an EmbodiedJev batch result. The report is stored as view state, so it survives reopening the view, and 导出报告 shows it as JSON to copy; the view cannot download files. A step below 决策门槛 ends that episode as 等待人工处理. Vision episodes need AsterDream, since each step is a model call.

<a id="desktop-view-policy"></a>

## AsterDream view policy

The view document is sandboxed (`allow-scripts`, opaque origin) with `connect-src 'none'` and no `'unsafe-eval'`. The page fetches nothing; scripts and fonts load as view files:

- `vendor/mujoco/wasm.js` carries the gzip-compressed WebAssembly as base64. `engine.js` inflates it in memory, and the policy's `'wasm-unsafe-eval'` lets the page compile it.
- `vendor/mujoco/mujoco.js` is patched so embind builds its invokers as closures, not with `new Function`.
- `vendor/g1/g1.js` carries the G1 station the same way: the physics XML, the quantized visual meshes, and the Panda hand collision meshes.

The upstream G1 collision meshes are STL files of tens of megabytes. MuJoCo collides against a mesh geom's convex hull, so `tools/vendor-g1.mjs` writes each one into the physics XML as the vertices of the hull MuJoCo itself builds (computed with the same WebAssembly build, then compiled again to check that the hull, its bounding box, and its bounding radius do not change). The page puts no G1 file into MuJoCo's virtual file system.

The vendor tools regenerate these files: `tools/vendor-mujoco.mjs` (MuJoCo; refuses an upstream file the patch does not match), `tools/vendor-g1.mjs <assets root>` (the G1 station from the unpacked `g1.zip` layout: `g1/` and `panda/`), and `tools/vendor-viewer.mjs` (three.js, Lucide icons, Ubuntu fonts).

## Tests

```sh
cd apps/use-registry/packages/g1-arm
node --test tests/*.test.mjs
node tests/browser.proof.mjs
node tests/load.proof.mjs
```

The unit tests cover the agent protocol, the world (upstream travel pose, degrees of freedom, mass, colliders), the planner, the session, perception, the vision menu, batch evaluation, and the package shape. The browser proof loads the view in Chromium under AsterDream's exact view policy and sandbox, and drives the real `agent/run.mjs` through the host turn loop. It runs System One replies at 0.8, a 401 gateway (the agent uses its rule), a refused agent turn (the page uses its rule and labels it), the three tasks on RGB-D estimates, two vision episodes (a scripted answer that stalls, and an unoffered answer that must stop), System One replies at 0.4, which must stop in 等待人工处理, and a 12-episode batch whose stored report must survive a reload. It needs the Playwright install in `apps/desktop`.

The load proof measures navigation to the first robot frame, phase by phase, in Chromium, in WebKit when Playwright has it, and on macOS with `swift` in WKWebView on the GPU (`tests/webview-load.swift`). It fails when a median exceeds 1000 ms (`LOAD_BUDGET_MS`).

`tests/desktop-open.swift` measures the same load in a running AsterDream window, by hand: it opens the installed workbench from the activity bar and times the press to the first robot frame on screen.

The end-to-end test runs AsterDream's real authority: catalog install, view session, asset route, view-agent turn route, a warm `a3s-box`, and the member gateway. `tests/desktop.e2e.mjs` plays the renderer in Chromium. It is the ignored Rust test `g1_arm_runs_end_to_end_through_desktop` in `apps/desktop/src-tauri`. Without environment it serves a fake gateway; with `A3S_DESKTOP_E2E_GATEWAY` and `A3S_DESKTOP_E2E_MEMBER_SESSION` it runs against a real one, and the member token stays in Rust.

`tests/PLAN.md` is the functional test plan. It maps each case to the test that proves it.

`tests/`, `tools/`, and `brand/` are package source only. The API publishes an explicit file list (`apps/api/src/contexts/plugin/infrastructure/g1_arm_package.rs`) that excludes them; its test fails when this directory and the list differ.

## Supply

This directory is package source. The admission is `a3s/g1-arm` in `admissions.acl`. Operators publish it with `a3s-use-registry-tools assemble`. The API seeds the same files as the catalog plugin `a3s-g1-arm` (`workbench_seed.rs` builds the archive for each robot workbench).

Third-party notices: `ui/workbench/THIRD_PARTY_NOTICES.md`.
