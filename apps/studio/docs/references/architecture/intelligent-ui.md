---
description: >-
  Boundary between AIX plugin UI contributions and model-generated Intelligent
  UI, whose current rendering language is OpenUI Lang
sources:
  - rsbuild.config.ts
  - src/renderer/components/chat/messages/markdown/OpenUiMessage.tsx
  - src/renderer/components/chat/messages/markdown/intelligentUiHost.ts
  - src/shared/types/agentForm.ts
  - src-tauri/src/agent_plane/forms.rs
---

# Intelligent UI and AIX UI

Studio has two independent contracts.

| Contract | Producer | Host responsibility | Consumer |
|---|---|---|---|
| AIX UI | Installed, admitted AIX Package | Contribution registration, package/version identity, capabilities, scoped lifecycle and withdrawal | ActivityBar, views, panels, workspace, mini-apps, Composer |
| Intelligent UI | Model-generated source | Restricted interpretation, closed components, local values, source version and explicit submission | Message cards and pending agent forms |

OpenUI Lang is the current source language for Intelligent UI. It is not the
AIX package format or its plugin contribution protocol. An OpenUI program
cannot register a view, replace Studio UI, invoke native commands or obtain
plugin permissions. AIX remains an ACL-described `.aix` archive.

`@a3s-lab/intelligent-ui` owns the renderer and closed catalog.
`OpenUiMessage` adapts OpenUI Lang into that renderer. A message fence may use
a conversation continuation; a pending form uses an exact owner/version-bound
form submission. Explicitly missing form submission never falls back to a new
conversation turn. Historical forms without a current binding remain source.

The separate `packages/intelligent-ui-aix` adapter distributes the authoring Skill and standalone
document. That distribution identity does not make Intelligent UI and AIX UI
one protocol. `packages/intelligent-ui` has no AIX packaging directory or manifest.
OpenUI statements become a bounded component tree directly; MDX is not an
intermediate language or supported interaction source. The standalone Intelligent UI render port and an AIX workbench
document/provider port are separate endpoints; neither inherits the other's
capabilities. The built-in catalog does not execute arbitrary plugin HTML.

New pending forms declare `format: "openui"`. Retired `aix-ui` source tags and
fences do not establish a live submit lease. Binding metadata describes the
current execution and exact source; native resource grants remain independent.

See [AIX UI document protocol](./aix-ui-document-protocol.md) for plugin documents
and [Agent Forms](../ai/agent-forms.md) for same-turn submission and verification.
