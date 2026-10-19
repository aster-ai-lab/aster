---
description: >-
  OpenUI Lang forms inside messages: ask_user_form parks the run, the session
  catalog renders the source, and typed submitted values resume the same turn
sources:
  - src-tauri/src/agent_plane/forms.rs
  - src-tauri/src/agent_plane/turns_form_tests.rs
  - src/shared/types/agentForm.ts
  - src/renderer/components/chat/form/FormPartView.tsx
  - src/renderer/components/chat/messages/markdown/OpenUiMessage.tsx
  - scripts/test-owned-agent-forms.sh
---

# Agent Forms

## Runtime chain

1. The builtin `ask_user_form` tool accepts OpenUI Lang in its `form`
   parameter. The source assigns `root` and has no Markdown fence. The tool
   checks that envelope and the size bound, then parks the run through the
   existing question machinery.
2. ACP sends `_a3s/form/request` with `formId`, `prompt`, `format: "openui"`
   and the OpenUI Lang string in `spec`. The request waits until submitted or
   cancelled. Studio refuses an untagged new request instead of parking on
   retired JSON.
3. The host assigns a random request ID, independent of the agent's `formId`.
   A TurnService-owned registry binds it to Session, Execution, attempt, anchor,
   ACP session and prompt. Native prompt-correlated Run observations are included
   when available. The revision is SHA-256 of the exact source bytes.
   The plane publishes standard AI SDK `data-form` chunks with
   `{type, id, data: {id, format, spec, state, binding}}` and checkpoints the same
   part. `OpenUiMessage` uses Intelligent UI to render the source inside the session tree.
4. Required fields are checked locally. Typed named answers pass through
   `AgentFormSubmitContext` to authenticated `POST /api/agent-forms/{id}`
   with `{binding, values}`. The version-1 binding includes the host request ID,
   full owner and source revision. The host requires the exact current binding
   and resolves it atomically once. The response is a correlated
   `{protocolVersion, requestId, ok}` receipt. The plane records and emits the
   answered summary, then returns values over ACP.
5. The values become the tool result through the fact answer; the same agent
   turn continues. Cancelling revokes the owner's pending form. A closed channel
   returns cancellation, never a successful empty answer. `awaiting-form` keeps
   the execution active and visible during reconnect.

## Message UI

An `openui` fence uses the same catalog renderer inside ordinary and task
messages. A closed statement can paint before the fence ends. Inputs keep
their values when source is appended or the fence closes. A fence does not
submit and does not start a user turn. Submit is enabled only for a bound
pending form. Those values return to the same tool call.

The packaged `intelligent-ui` Skill and bundled `ask-form` Skill describe the component
catalog. A completed `json` fence is a form only when its entire body is one display
document with `state.form`. The card title is `title` when present, otherwise `root`.
Each answer name is that field's own key. A label comes from `state.labels` or from
`{ "label", "value" }`; without one, the label is the key. A key that is not an
identifier stays source, and the fence is not searched for a card inside prose.
An untagged pending form displays source only. Previously answered parts retain
their submitted summary.

## Boundaries

These are Intelligent UI forms, not AIX workbench contributions. Their source
format is `openui`; ActivityBar/view/mini-app protocols and plugin capabilities
remain owned by AIX UI. See [Intelligent UI](../architecture/intelligent-ui.md).

- Model source never runs as JavaScript. Imports, event handlers, DOM tags,
  function calls and member access are rejected by the restricted interpreter.
- The session renderer uses the closed catalog. It does not load a model-supplied
  document, and it has no network access.
- A pending form's submit channel never falls back to a conversation continuation.
  This prevents a second agent turn from being created while the first is parked.
- The renderer verifies the exact source digest before submission. Owner or source
  replacement resets local input and aborts the old card, including source ABA.
  Saved unbound requests display source only; history never gains a new lease.
- Requests have at most 64 scalar fields and 64 KiB serialized values; strings
  have a 4096-byte bound. Null, nested objects and non-finite/out-of-range numbers
  are refused. The registry admits at most 128 pending requests.
- Run identity in a form binding is an ownership observation, not a resource grant.
- Answered forms are read-only.

## Verification

From `apps/studio`, supply a new retained evidence directory:

```sh
bash scripts/test-owned-agent-forms.sh /private/tmp/a3s-owned-agent-form-proof
```

The script produces a real native binding, checkpoint, SDK chunks, submission and
receipt, then runs them through the installed AI SDK and actual task UI. The
renderer exercises the data submission adapter with a controlled network boundary.
This certificate does not prove real native HTTP, ACP tool execution or platform
WebView isolation. Those require separate live campaigns.
