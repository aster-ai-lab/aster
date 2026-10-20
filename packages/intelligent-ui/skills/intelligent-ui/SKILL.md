---
name: intelligent-ui
description: Show an interactive calculator, comparison, chart or input form directly inside the conversation, using AsterDream Intelligent UI.
---

Emit a fenced `openui` block when an interactive answer helps the user. `ask_user_form` takes the same source without a fence. Each line is `name = Component(args)`. Arguments follow the prop order below. The first statement assigns `root`. Child components are identifier lists. This is not executable JavaScript. A closed line can render before the fence ends. Do not invent another interface. JSON, MDX, and HTML are source, not a card. Do not copy a tool result into a json or openui fence. The host decides whether that data is a form, a view, or prose. If a card is already on screen, refer to it in one sentence. Author an openui fence only for an interaction you compose yourself.

```openui
root = Card("旅行预算", [days, daily, total, submit])
days = Slider("days", "天数", 5, 1, 14, 1)
daily = Input("daily", "每天预算", 300, "number", 0, 10000)
total = Metric("合计", {days * daily}, "元")
submit = Button("submit", "使用这个预算")
```

Components and props:

| Component | Props |
| --- | --- |
| Card | title |
| Stack | — |
| Grid | columns (1–3) |
| Text | — |
| Input | name, label, initial, type (`text`, `number`, `email`, `tel`), min, max, step, required, placeholder |
| Textarea | name, label, initial, required, placeholder |
| Slider | name, label, initial, min, max, step |
| Select | name, label, initial, options (array of unique nonempty strings), required, placeholder |
| Checkbox | name, label, initial (boolean), required |
| Metric | label, value, unit |
| BarChart | labels (array of strings), values (array of numbers) |
| Button | action (`submit`), label |

Every input has a unique `name` (identifier). Input names become variables in expressions. Expressions may contain literals, arrays, objects, input identifiers, arithmetic, comparisons, logical operators and ternaries. Calls, member access, assignment, functions, imports, exports, spreads, DOM tags and event handlers are rejected. Required text fields must be nonempty and required checkboxes must be checked before submitting. Expressions cannot access the network or host. Do not request secrets.

Changes calculate locally immediately. A fence does not submit. Submit is enabled only for a bound pending form. Those values return to the same tool call. Treat them as the confirmed input. Do not repeat the form. The plugin does not call tools, send requests, or continue a conversation by itself.
