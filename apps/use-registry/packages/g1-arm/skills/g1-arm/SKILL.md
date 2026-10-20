---
name: g1-arm
description: Choose one offered phase or action for the 宇树 G1 人形机器人 workbench from one observation.
version: 1.0.0
---

# 宇树 G1 人形机器人

The `a3s/g1-arm` workbench runs a MuJoCo simulation of the right arm of a Unitree G1 humanoid inside the AsterDream view. The torso is fixed at the table edge, and the arm carries a Panda parallel gripper. There are three tasks: put the red cube in the blue tray (搬运入盘), stack it on the blue cube (方块堆叠), or carry it over a barrier into the tray (越障搬运). Each task ends with the gripper open and withdrawn. Each cycle has two decisions: first a phase, then one of the previewed actions for that phase. You choose one offered key per decision. You do not claim a physical certification.

## Input

- `state.observation`: the `robot` name, the measured `tcp`, `object`, `destination`, gripper and contact state, relative `geometry`, `relations`, `forbidden_contact`, and `success`. Units are metres.
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

## Vision observations

When the observation mode is 视觉模型, each decision is one step from a fixed menu of 21 keys: translations of 40, 10, or 2 mm along ±x, ±y, ±z, and `open`, `close`, `hold`. The state has the measured gripper, sensors, both camera calibrations, the goal conditions, and the last six outcomes; it has no object pose and no `success`. Two labelled camera views, `external` and `wrist`, come with the turn.

Reply with one JSON object:

```json
{"choice":"<one offered key>","intent":"<optional, at most 240 characters>","visual_evidence":"<optional, at most 240 characters>"}
```

1. Ground the choice in what the views show. Do not claim details that are not visible.
2. `intent` and `visual_evidence` are short public summaries, not a reasoning trace or a plan of later steps.
3. A move that would leave the workspace is not offered. A step whose preview touches the table or barrier is refused and recorded, and the next decision sees that outcome. An unoffered key or a malformed reply stops the episode. Nothing replaces a refused or failed choice.
