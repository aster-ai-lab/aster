---
name: cobot-arm
description: Choose one offered phase or action for the 七轴轻量化协作机器人 workbench from one observation.
version: 1.0.0
---

# 七轴轻量化协作机器人

The `a3s/cobot-arm` workbench runs a MuJoCo simulation of a Franka Panda arm inside the AsterDream view. There are three tasks: put the red cube in the blue tray (搬运入盘), stack it on the blue cube (方块堆叠), or carry it over a barrier into the tray (越障搬运). Each task ends with the gripper open and withdrawn. Each cycle has two decisions: first a phase, then one of the previewed actions for that phase. You choose one offered key per decision. You do not claim a physical certification.

## Input

- `state.observation`: the measured `tcp`, `object`, `destination`, gripper and contact state, relative `geometry`, `relations`, `forbidden_contact`, and `success`. Units are metres.
- `state.recent_outcomes`: the last two executed actions and their displacement.
- `decision.instructions`: the question for this decision.
- `options`: the offered keys. An action value is a JSON string with the target, gripper command, duration, and simulation preview.
- `baseline_choice`: the rule controller's choice.

## Output

Reply with one JSON object and no other text:

```json
{"choice":"<one offered key>"}
```

`choice` must be exactly one key of `options`.

## Rules

1. Use only fields present in the observation. Do not invent coordinates, contact, or camera pixels.
2. Prefer `baseline_choice` when the measurement agrees with the task. Do not repeat a motion that `recent_outcomes` shows had no effect.
3. Do not add `probabilities`. This skill has no probability source.
4. Do not copy API keys, tokens, or authorization headers into the reply. Model calls go through the a3s gateway; the member does not paste a supplier key.

## LIBERO

The workbench's LIBERO tasks (关闭顶层抽屉, 关闭微波炉) do not use this skill. Their turns carry `observation.protocol = "libero-rgbd-waypoint-v1"` and the `external` and `wrist` camera images. The package agent sends them to a vision chat model with the upstream planner prompt, and the model answers one waypoint plan. When that fails, the episode stops; no offered key or rule replaces the plan.
