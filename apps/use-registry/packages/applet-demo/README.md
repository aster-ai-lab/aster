# Aster Applet Demo

Signed example package that supplies an Applet UI surface (`ui` /
`panel`) with Skill, stdio MCP, and package-local Executable Tool binds for
host projection proofs.

The package ships Executable Tool Task `echo` so Skill readiness, UI
`bind_tool`, and registry planning cover the Tool surface. Hosts project that
Tool via file-evidence reinspect (not a Runtime BindingStore receipt).

This package is **supply only**. Hosts project and render it; the Registry does
not embed Desktop UiHost or Code projection logic.

## Surfaces

| Surface | Id | Role |
| --- | --- | --- |
| Tool Task | `echo` | Package-local executable; Skill-required and UI-bound |
| MCP stdio | `context` | Package-local NDJSON MCP bound by the UI |
| Skill | `applet-demo` | Documents the demo; requires Tool + MCP |
| UI | `panel` | Content-addressed HTML/CSS/JS entry (binds Tool + Skill + MCP) |

## Authoring rules (summary)

See [docs/applet-ui-package-supply.md](../../docs/applet-ui-package-supply.md).

- Declare `ui` assets as package-relative paths; digests are computed at assemble.
- `bind_tool` / `bind_mcp` must reference ids that exist in this package.
- Incomplete dependency evidence fails closed at Use publish — do not ship UI
  that binds missing surfaces.
- Do not add Host projection, WebView, or renderer code to this repository.
