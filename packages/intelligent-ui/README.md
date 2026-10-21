# Intelligent UI

Intelligent UI renders model-generated interactions inside AsterDream messages.
Its current source language is OpenUI Lang: `name = Component(args)`.
A completed `json` fence is a form only when its body is a display document with `state.form`. That document is projected into OpenUI and painted with this catalog. Other JSON, MDX, and HTML stay source.

This is separate from AIX UI, the plugin workbench contribution and document
protocol for ActivityBar, views, panels, workspaces and mini-apps. AIX UI owns
package admission, host capabilities and lifecycle. Intelligent UI owns a
closed component catalog, local values and explicit form/continuation channels.
An Intelligent UI submission never grants plugin capabilities or registers a
workbench contribution. The optional AIX distribution lives in the separate
[`intelligent-ui-aix` adapter](../intelligent-ui-aix/README.md). This library has
no AIX manifest, installation logic or workbench contribution protocol.

```openui
root = Card("Budget", [days, daily, total, submit])
days = Slider("days", "Days", 5, 1, 14)
daily = Input("daily", "Daily budget", 300, "number", 0)
total = Metric("Total", {days * daily}, "USD")
submit = Button("submit", "Use this budget")
```

## Build and deliver

From this directory, after the repository UI dependencies are available:

```sh
npm install --workspaces=false --ignore-scripts
npm test
npm run typecheck
npm run build
```

Build emits ESM renderer/parser modules in `dist/`. It does not create an AIX
archive. AsterDream imports the library directly; rendering an OpenUI message
does not require installing a plugin. The optional adapter has a separate build.

<a id="desktop-integration"></a>

## AsterDream integration

- `openui` fences appear inside ordinary and task messages. A closed statement
  can paint before the fence ends. Plain `mdx` fences remain code examples.
- Inputs, Textarea, Select, Checkbox and Slider calculate locally. Required
  controls are validated before submission. Metric and BarChart expressions
  reference named input values.
- A fence paints the catalog. It does not start a user turn. Submit is enabled
  only for a bound pending form, and those values return to that tool call.
- `ask_user_form` accepts OpenUI Lang. ACP requests and saved/streamed form parts
  carry `format: "openui"`. Their values resolve the addressed pending form and
  resume the same agent turn. Answered parts display a read-only summary.
- Live pending forms require a host-owned request binding and exact source
  revision. Unbound history stays readable source; its submit channel cannot
  become a next-turn continuation. Replacing a form cancels the old card.
- A completed `json` fence with `root` and a non-empty `state.form` renders as a
  form after the fence closes. Incomplete JSON stays a code block. An untagged
  pending form part displays source only.

The catalog and source rules are documented in
[the model authoring Skill](skills/intelligent-ui/SKILL.md). AsterDream renders `openui`
source with the exported catalog component in the session tree.

## Execution boundary

OpenUI statements become a bounded component document directly. There is no
MDX intermediate language or executable MDX export. Acorn parses expression
syntax; the closed interpreter permits only literals, named inputs, arrays/objects, arithmetic, comparisons,
logical operators and ternaries. It rejects imports, exports, DOM tags, event
handlers, calls, member access, assignments, functions and spreads.
Repeated statements, missing complete-document references and cycles are refused.
During streaming, children whose statements have not arrived are omitted. It never
compiles or evaluates generated JavaScript.

Bounds: 64 KiB source, 160 AST nodes, 20 component nesting levels, 64 controls,
4096 characters per answer. Inputs are transient while mounted; answered pending
form values are persisted by the existing agent plane. Calculator drafts and
next-turn submission status are not persisted across message remounts.

## Verification

`npm test` checks OpenUI calculations, bounded expansion, literal text and denied
source. `npm run typecheck` checks the renderer types. AsterDream's message/form tests
check user interactions and same-turn submission. Native package admission and
archive/browser checks belong to the [AIX adapter](../intelligent-ui-aix/README.md).

For the independent renderer browser fixture, from `apps/desktop` in separate
terminals run `node tests/e2e/intelligent-ui/serve.mjs` and
`node tests/e2e/intelligent-ui/verify-review.mjs`. It loads no AIX package and
uses controlled submission channels. It requires a local socket and A3S Test;
it does not certify native form ownership or plugin installation.
