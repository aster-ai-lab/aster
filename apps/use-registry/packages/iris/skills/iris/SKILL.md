---
name: iris
description: Choose one offered phase or action for the 3DR iris 四旋翼 workbench from one observation.
version: 1.0.0
---

# 3DR iris 四旋翼

The `a3s/iris` workbench runs a MuJoCo simulation of a 3DR Iris quadrotor inside the AsterDream view. There are three missions: fly a patrol and land (空中巡航), hook a payload and drop it in a zone (外卖配送), or hold heading at three standoff points (设施巡检). Each mission ends on the landing pad, rotors stopped, airframe upright. Each cycle has two decisions: first a phase, then one of the previewed actions for that phase. You choose one offered key per decision. You do not claim a physical certification.

## Input

- `state.observation`: the `robot` name, the measured `tcp` (the airframe position), `object` (the next mission point), `destination`, hook and contact state, relative `geometry`, `relations`, `forbidden_contact`, `success`, and `mission`. Units are metres. `mission` carries the task, pad, marks or inspect points, payload, and whether the airframe is grounded.
- `state.recent_outcomes`: the last two executed actions and their displacement.
- `decision.instructions`: the question for this decision.
- `options`: the offered keys. An action value is a JSON string with the target, hook command, duration, and simulation preview.
- `baseline_choice`: the rule controller's choice.

## Output

Reply with one JSON object and no other text:

```json
{"choice":"<one offered key>"}
```

`choice` must be exactly one key of `options`.

## Rules

1. Use only fields present in the observation. Do not invent coordinates, contact, or camera pixels. This workbench does not send images.
2. Prefer `baseline_choice` when the measurement agrees with the task. Do not repeat a motion that `recent_outcomes` shows had no effect.
3. Do not add `probabilities`. This skill has no probability source.
4. Do not copy API keys, tokens, or authorization headers into the reply. Model calls go through the a3s gateway; the member does not paste a supplier key.
