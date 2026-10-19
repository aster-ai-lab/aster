# Read-only model selection and restart audit

Both explicitly authorized successful turns were configured as `glm::glm-5.3`
in the host's typed assistant records and per-message model snapshots.
The subsequently observed shared agent model is `anthropic::qwen3.8-flash`.
This distinguishes the actual submitted-turn model record from a current
composer label. No product source, setting, model selection, build or host was
modified in this audit.

## Bounded observed metadata

Only the following two message files were projected, with a 20 MiB per-input
bound, plus the model field of the explicitly authorized agent ID. No other
session was inspected. [whitelisted-model-metadata.json](whitelisted-model-metadata.json)
contains only assistant identity, status, timestamps and model metadata:

| Session | Assistant anchor | Host model ID | Snapshot model/provider | Status |
| --- | --- | --- | --- | --- |
| 01a15187-2822-7567-a7b5-926622986293 | 01a15187-28fc-7245-a227-28f357d89a9f | glm::glm-5.3 | glm-5.3 / glm | success |
| 01a15188-c26a-71e1-a441-b9f24dff66a8 | 01a15188-c36f-76f5-8d0a-b5abda47ff10 | glm::glm-5.3 | glm-5.3 / glm | success |

At both bounded observations the authorized shared agent
`01a14deb-5f83-75b0-8d92-25be3c3589fb` had top-level model
`anthropic::qwen3.8-flash`; configuration.model was absent. The real field used
by the host is the agent's top-level `model`, rather than configuration.model.

## Source chain

- `WorkbenchModelPicker.choose` resolves the selected model; on the local shell
  it awaits `writeShellModelDefault`, then writes `chat.default_model_id`, then
  awaits the supplied `onSelect`. The shell-default write POSTs only model
  references to `/api/providers/defaults`; no key is in that selection body.
- `useWorkbenchModel` prefers a filled compatible-settings model if present;
  otherwise an explicit chat-default preference resolves via `useDefaultModel`.
  With no explicit preference, it uses the existing catalog/shell/default list
  fallbacks. No actual preference or compatible-settings value was read here.
- The workbench chip uses `rememberedModel ?? model`. Selection invokes
  `AgentComposer.handleModelSelect`, which PATCHes the shared agent's `model`
  through `useUpdateAgent.updateModel`. A remembered-model effect also applies
  the current default to that shared agent after hydration/restart.
- New sessions reference the agent; they do not freeze a model in their create
  body. Normal `AgentComposer.sendQueuedPayload` and
  `useAgentChatRuntimeState.buildStreamRequest` currently omit a per-turn
  `model` field. The host selects explicit body.model when present, otherwise
  the agent model, in `agent_plane/turns.rs` around lines 557–575.
- The host creates the assistant's `modelId` and model snapshot from that same
  selected value around lines 733–750. It passes the selected value to
  `runtime`/`launch`; runtime reuse requires exact model equality, otherwise
  the previous sidecar is replaced. Launch splits the selected provider/model
  and supplies that model to ACP. A later shared-agent change does not rewrite
  the historical assistant model snapshots.

These records therefore establish successful host turns configured for GLM,
not just a GLM chip. This audit did not inspect a network wire trace or an
upstream provider's private routing, so it does not assert an independently
observed upstream response-model identifier.

## Restart behavior and minimal follow-up

The debug startup source contains an explicit replay policy:

1. `ProviderStore::open_at` calls `apply_dev_acl`.
2. In non-test debug builds, that applies a configured development seed and
   persists it. `apply_seed` replaces all three model-default slots when the
   seed has a default; it does not retain an explicit UI-picked default.
3. `lib.rs` startup calls `publish_dev_chat_default` in the same debug-only
   branch. That opens the provider store and writes its current default to the
   device `chat.default_model_id`, even when a different preference was saved.
   Its own comment states that the next debug launch applies the configured
   default again. Release builds do not execute this function.
4. The rehydrated `useWorkbenchModel` then shows that default, and the Composer
   synchronization effect can PATCH the shared agent to it. This accounts for
   a restored SH02 reply retaining GLM metadata while the current chip and
   shared agent read Qwen after restart.

The normal `just build-work` recipe is a debug build. This is a concrete
source-level reset mechanism consistent with the operator's restart and the
observed agent transition; it is not evidence that the selector forgot to
persist its GLM choice. No actual development configuration or override value
was inspected, so the configured Qwen value itself was not independently
verified. The debug policy is explicit in source; its conflict is with the
product expectation that a user's explicit UI model choice survives restart.

The smallest later change to consider is preserving an existing explicit
model choice during development seeding and publishing the development
preference only when that preference is absent. Both default-store reseeding
and preference replay should be covered by a small restart regression, rather
than relying only on a chip assertion. Root requested this as a recorded
follow-up; no implementation or extra packaging was started during the corpus
run.

## Explicitly not claimed

- No credentials, ACL/configuration contents, provider store values, runtime
  process arguments, environment values, prompt/reply/attachment contents, or
  other session metadata were output or inspected for this audit.
- No source or test edit, model/API request, CUA operation, configuration write,
  build, deployment, restart or existing task score was changed.
- No model-selector fix, cross-restart guarantee or live provider wire-level
  verification is claimed. The exact two host model snapshots and current
  authorized agent model are the observed runtime evidence.
