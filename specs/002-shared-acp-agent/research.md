# Research: interactive `a3s/` methods on initialize and one prompt

Source of the wire, read from the functions that build the messages. A name that only appears in a handler, a comment, or a later command is not in this list.

## Initialize

`client_capabilities_meta` in `crates/terminal/crates/codegen/a3s-code-pager/src/acp/mod.rs` inserts these keys into `InitializeRequest` client capabilities. `build_initialize_meta` adds `clientType` and `clientVersion` and does not add an `a3s/` key.

| Key | Who sends it | Shared agent | Classification |
| --- | --- | --- | --- |
| `a3s/incrementalBashOutput` | client advertisement, boolean true | `crates/code/acp` has no read of this key | Client does not depend on an agent implementation. Connect continues when the agent omits it. |
| `a3s/hunkTracker` | client advertisement, object `mode` | no read | Client does not depend on an agent implementation. |
| `a3s/bashOutputNoColor` | client advertisement, boolean true | no read | Client does not depend on an agent implementation. |
| `a3s/gitHeadChanged` | client advertisement, boolean true | no read | Client does not depend on an agent implementation. Inbound `a3s/git_head_changed` is a different method and is not sent on initialize. |
| `a3s/userMessageEcho` | client advertisement from `USER_MESSAGE_ECHO_CAPABILITY` | no read | Client does not depend on an agent implementation. |
| `a3s/statusLine` | client advertisement from `STATUS_LINE_CAPABILITY` | no read | Client does not depend on an agent implementation. The comment says the agent may skip an unpainted payload. Absence does not fail initialize. |

No behavior change is required for these six. The hermetic test `initialize_xai_advertisements_are_the_closed_set` locks the names. Adding a seventh key fails that test until this inventory is updated in the same change.

## One ordinary prompt

`prompt_request_meta` builds `PromptRequest._meta` for `Effect::SendPrompt`. The keys are `promptId` and, when the screen mode is set, `screenMode`. Neither name starts with `a3s/`.

`plain_prompt_content_block` adds `skillTokenRanges` on the text block only when ranges are non-empty. That name is not an `a3s/` method.

The hermetic test `ordinary_prompt_meta_sends_no_xai_extension` locks this.

## Not on this wire

These names are handled or sent by other commands. They are not part of initialize or one ordinary prompt, and this epic does not reclassify them:

- `a3s/session_notification` and `a3s/session/update` are inbound session updates. The client matches them in `is_session_update_ext_method`. It does not send them on initialize or on `Effect::SendPrompt`.
- `a3s/session/prompt_complete`, `a3s/turn/outcome`, and the command catalog in `app/effects/mod.rs` are separate requests or notifications.

The shared agent already implements `a3s/session/info`, `a3s/session/list`, `a3s/session/delete`, `a3s/permission_posture`, `a3s/compact_conversation`, and `a3s/interject` in `crates/code/acp/src/agent/host_methods.rs`. Those methods are not sent by the initialize or ordinary-prompt builders above, so this epic does not add conformance cases for them.
