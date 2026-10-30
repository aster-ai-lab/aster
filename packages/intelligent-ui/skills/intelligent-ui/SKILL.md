---
name: intelligent-ui
description: Show a JSON Render card, chart or structured input form directly inside an Aster conversation.
---

Use one JSON Render document with `root`, `elements` and optional `state.form`. Each element has `type`, named `props`, and optional `children` containing element IDs. Emit a `json-render` fence for a card in a reply. To wait for user input, call `ask_user_form` with this JSON document encoded as its `form` string, without the fence.

Bind input `value` (Checkbox `checked`) to `{"$bindState":"/form/name"}`. Names are unique ASCII identifiers, at most 48 characters. Initial values in `state.form` are strings, finite numbers or booleans. Missing defaults are empty text, zero or false. Submit is the only action. Required text must be nonempty; required checkboxes must be checked. Select options are unique nonempty strings. Numeric bounds use `min`, `max`, `step`.

| Component | Props |
| --- | --- |
| BarChart | labels, values |
| Button | action, label |
| Card | title |
| Checkbox | label, checked, required |
| Form | submitAction |
| Grid | columns |
| Input | label, value, type, min, max, step, required, placeholder |
| Metric | label, value, unit |
| Select | label, value, options, required, placeholder |
| Slider | label, value, min, max, step |
| Stack | direction, gap |
| Text | content |
| Textarea | label, value, required, placeholder |

```json-render
{
  "root": "root",
  "elements": {
    "root": {"type": "Card", "props": {"title": "补全信息"}, "children": ["topic", "submit"]},
    "topic": {"type": "Input", "props": {"label": "主题", "value": {"$bindState": "/form/topic"}, "required": true}},
    "submit": {"type": "Button", "props": {"action": "submit", "label": "继续"}}
  },
  "state": {"form": {"topic": ""}}
}
```

The host validates the complete JSON document before rendering. Streaming incomplete JSON stays pending. Components are closed: no imports, scripts, DOM tags, event handlers, executable expressions, arbitrary actions or host/network access. Text is literal. Do not request secrets. Titles, labels, placeholders and option labels follow the user's language; field names remain ASCII identifiers.

Message fences display cards but have no submit lease. Only a currently bound pending form can submit, and its typed values return to the same tool call. Do not start another conversation turn. If structured tool data is already displayed by the host, refer to it without copying its JSON or drawing another card.
