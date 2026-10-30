# Intelligent UI

Aster renders JSON Render documents directly inside messages and pending agent forms. The only component protocol is `root` / `elements` / `state`, with named props and `$bindState` field bindings. The closed catalog is generated from `a3s-form-core json-render-catalog`.

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

`json-render` fences appear in ordinary and task messages. A completed `json` fence may contain the same document, or an explicitly declared data form with `root` and `state.form`; the latter is projected directly into JSON Render. Other JSON and source languages remain code.

`ask_user_form` uses the same JSON document without a fence. ACP requests and saved/streamed form parts carry `format: "json-render"`. Only a live host-owned binding enables submission. Owner identity, exact source SHA-256 and single-use consumption bind answers to the waiting tool call. Answered forms are read-only; unbound history cannot submit. Draft values are local to the mounted card.

The interpreter validates the complete JSON document before rendering. Incomplete streamed JSON waits; appending bytes and closing the fence preserve mounted input state. No generated JavaScript, expression evaluator, arbitrary HTML, remote assets, native commands or plugin capabilities are available. Bounds include 64 KiB UTF-8 source, 160 components, 20 nesting levels, 64 fields and 4096 bytes per string answer.

The optional [AIX adapter](../intelligent-ui-aix/README.md) distributes the authoring Skill and standalone document. Aster imports this library directly and does not require plugin installation. AIX workbench contributions and resource permissions remain separate.

From this directory: `npm test`, `npm run typecheck`, `npm run build`. Tests cover JSON interpretation, source rejection, bounds, typed values and data/schema projection. The shared fixture is also checked by native Rust and terminal tests. Live desktop/ACP and AIX installation require their own campaigns.
