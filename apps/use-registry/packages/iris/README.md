# 3DR iris 四旋翼

Package `a3s/iris@1.0.0`. One self-contained `.aix` package, three surfaces:

| Surface | Id | Role |
| --- | --- | --- |
| Skill | `iris` | Reads one observation and returns exactly one offered `choice`. |
| Agent | `iris` | Edge package harness, entry `agent/run.mjs`, 500 cpu millis, 256 MiB. AsterDream keeps one warm `a3s-box` per open view. |
| UI | `workbench` | Mini app (`desktop.applets`): no Activity Bar entry; after install, 打开 opens it in its own system window. Root `ui/workbench`. |

Nothing runs on the host outside AsterDream. There is no lab server, no port, and no Python.

The package is the EmbodiedJev Iris integration (`iris.zip` at the repository root) ported onto the same workbench form as `a3s/g1-arm`: the same page shell, decision loop, edge agent, and view policy. The vehicle and the three missions are the quadrotor's.

## Workbench

- `world.js` builds the Iris of EmbodiedJev `IrisSpec`: mass 1.52 kg, inertia 0.029125 / 0.029125 / 0.055225 kg·m², four rotors at the PX4 motor positions, thrust 0–14 N, and a cascade position controller at a 0.002 s step. Visual meshes are the PX4 Iris frame and propellers, packed in `vendor/iris/iris.js`. Collision is a box. The hook is a data attachment, not a contact grasp.
- The three missions are 空中巡航 (three marks, then land on the pad), 外卖配送 (hook the payload, carry it, release it in the drop ring, then land), and 设施巡检 (three standoff points facing the tower, then land). Physics is the official MuJoCo 3.15.0 WebAssembly build in `vendor/mujoco/`.
- `planning.js` offers the one phase that applies to the current state and previews each candidate on a copy of the world. A candidate that crashes, fails to land, fails to hook or release, or misses the preview target is refused.
- `session.js` runs one episode: choose a phase, preview candidates, choose an action, execute it. An episode is verified when the physical success condition holds. It stops after 30 actions or three repeated actions with no displacement.
- `robot-scene.js`, `meshes.js`, and `app.js` draw the scene with three.js (Z up) and run the page.

The rule baseline completes 空中巡航 in 6 cycles. The same controller completes 外卖配送 and 设施巡检.

The page uses the simulator's privileged state only. The source cameras and the RGB-D / vision observation modes are not in this package.

## Decisions

Each cycle has two decisions: a phase, then an action. When only one key is offered, the page takes it without a call. Otherwise it sends the observation to the package agent with `aix.agent.invoke`. AsterDream runs `agent/run.mjs` in a warm edge box for the life of the view:

1. The agent emits a `model_request`. For the default decision model `platform/apofasi`, the request has `api: "systemone"` and a System One body: one `choice` question whose criteria are exactly the offered keys. For any other model given in `parameters.model`, it is a chat-completion body. AsterDream checks the body, calls the a3s gateway (`/v1/systemone` or `/v1/chat/completions`) with the member session, and posts the reply to `/v1/agent/model-responses`. The box never holds a credential.
2. A System One reply counts only when it chooses an offered key and publishes a probability for exactly the offered keys, summing to 1. The declared `apofasi-lexical-*` engine ranks text overlap; it cannot authorize a flight action. Iris refuses that result, uses the existing rule controller and explains why in the view. A chat reply counts only when it names one offered option. Otherwise, or when the gateway is unavailable, the agent uses the rule: the only option, else an offered `baseline_choice`, else the first option.
3. The output is `{"choice", "source": "model" | "rule", ...}`, plus `probabilities`, `confidence`, and the serving `model` from the decision model. The page refuses a choice it did not offer.

决策门槛 (default 0.55): when the chosen key's probability is below it, the episode pauses in 等待人工处理. 确认并继续 executes that reviewed action once; the next low-probability action requires another confirmation. A rule decision has no probability and never stops. Stop and reset cancel the page's pending decision wait. An unanswered host is bounded by the native cold-start plus turn budget (185 seconds); the page then labels its rule fallback as a timeout.

When the agent cannot run (for example, `a3s-box` is not installed), the page chooses with the same rule and labels the step `规则控制器（agent 不可用）` with the reason.

<a id="desktop-view-policy"></a>

## AsterDream view policy

The view document is sandboxed (`allow-scripts`, opaque origin) with `connect-src 'none'` and no `'unsafe-eval'`. The page fetches nothing; scripts and fonts load as view files:

- `vendor/mujoco/wasm.js` carries the gzip-compressed WebAssembly as base64. `engine.js` inflates it in memory, and the policy's `'wasm-unsafe-eval'` lets the page compile it.
- `vendor/mujoco/mujoco.js` is patched so embind builds its invokers as closures, not with `new Function`.
- `vendor/iris/iris.js` carries the quantized visual meshes the same way.

`tools/vendor-iris.mjs <assets directory>` regenerates `vendor/iris/iris.js` from the unpacked Iris STL files. The MuJoCo, three.js, Lucide, and font files are the same vendor set as `a3s/g1-arm`.

## Tests

```sh
cd apps/use-registry/packages/iris
node --test tests/*.test.mjs
node tests/browser.proof.mjs
```

The tests cover the flight world (mass, rest pose, crash, hook, the three missions on the rule controller) and the package shape. `tests/`, `tools/`, and `brand/` are package source only. The API publishes an explicit file list (`apps/api/src/contexts/plugin/infrastructure/iris_package.rs`) that excludes them; its test fails when this directory and the list differ.

The browser proof uses G1's view host with AsterDream's exact sandbox and CSP. Chromium and WebKit check controls during slow startup, actual scene pixels, reset and all three missions on the rule controller, one-time confirmation of low-probability actions, cancellation of an unanswered decision, and disabled controls when a mesh pack fails to load. It requires AsterDream's Playwright install and its Chromium and WebKit browsers. Set `PROOF_SCREENSHOTS` to retain the completed mission screenshots.

`IRIS_NATIVE_RUNTIME=<Desktop runtime receipt> IRIS_NATIVE_EVIDENCE=<output directory> node tests/native.proof.mjs` additionally verifies installed browser assets against this source and drives all three missions through a running AsterDream's view-bound edge agent and gateway. Iris must already be installed. The proof closes only its own view, uses G1's browser host, and records whether actions came from a planning model or the explicitly labelled refusal of a lexical engine; it does not claim a native-window GUI test or a neural-planning result for rule actions.

## Supply

This directory is package source. The admission is `a3s/iris` in `admissions.acl`. Operators publish it with `a3s-use-registry-tools assemble`. The API seeds the same files as the catalog plugin `a3s-iris` (`workbench_seed.rs` builds the archive).

Third-party notices: `ui/workbench/THIRD_PARTY_NOTICES.md`.
