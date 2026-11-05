# Translation loss ledger (PT0)

The frozen conformance corpus for OpenAI ↔ Anthropic chat translation lives
in `tests/fixtures/translation/`; `tests/translation_corpus.rs` locks its
shape. This ledger enumerates every field that cannot survive translation.
An unlisted loss is a contract violation — the translator must refuse or
explicitly drop, never silently mis-carry.

## Preserved in both directions

| OpenAI | Anthropic | Notes |
| --- | --- | --- |
| `messages[]` (user/assistant) | `messages[]` (content blocks) | Text content becomes block arrays. |
| leading `system` message | top-level `system` | Hoist is loss-free. |
| `tools[].function` | `tools[]` | `parameters` ↔ `input_schema`. |
| `tool_calls[]` | assistant `tool_use` blocks | `id` preserved; `arguments` JSON string ↔ `input` object. |
| `role:"tool"` turns | user `tool_result` blocks | `tool_call_id` ↔ `tool_use_id`. |
| `tool_choice:"auto"` | `tool_choice {type:"auto"}` | Named/required variants map to `tool`/`any`. |
| `max_tokens` | `max_tokens` | Required on the Anthropic side; the translator must pick a fail-closed default when absent rather than refuse silently — the default is pinned by the corpus. |
| `temperature`, `top_p` | same | Pass through. |
| `stop[]` | `stop_sequences[]` | Pass through. |
| multimodal content parts | content blocks | Image inputs translate (`image_url.url` base64 ↔ `source` base64); passthrough otherwise. |

## Dropped in the Anthropic direction (no equivalent)

- `logprobs`, `top_logprobs` — OpenAI response-side sampling detail.
- `presence_penalty`, `frequency_penalty` — OpenAI sampling penalties.
- `seed` — OpenAI determinism hint.

## Dropped in the OpenAI direction (no equivalent)

- `thinking` (extended-thinking budget) — OpenAI chat-completions has no
  equivalent surface; never fabricated.

## Refuse, never partially translate

- Unknown message roles, unknown content part types, malformed tool
  arguments (not a JSON object): the translator fails closed with a stable
  error naming the field; it never dispatches a partially translated body.
- Tool round-trips preserve the call id chain; a broken chain
  (`tool_result` without its `tool_use`) refuses.

## Responses translation under qualification

The public Gateway Responses route remains Reserved until T011/T018 in
`specs/030-magpie-compatible-local-gateway` qualify dispatch, native upstreams,
authorization and metering. The translation library alone is not that gate.

- Chat JSON and SSE output share one Responses item accumulator. Text,
  `reasoning_content`, refusal and indexed parallel function calls retain their
  distinct types, IDs, arrival order and argument fragments. Unknown delta
  fields, finish reasons and multiple choices refuse rather than disappear.
- A finish frame waits for trailing usage or `[DONE]`. Missing finish reasons
  refuse; `length` and `content_filter` produce `response.incomplete`. Cache
  input and reported reasoning details survive both JSON and streaming output.
- The published minimal stream-event API retains its frozen fixture output.
  HTTP SSE framing adds sequence numbers, content-part start/end events,
  text/refusal completion events and text annotations required by SDK clients.
- The HTTP body accepts LF/CRLF/CR and multiline `data` fields, bounds each
  event/line to 8 MiB, preserves typed errors, and rejects malformed/truncated
  streams. Protocol completion and client cancellation release the upstream.
  The Responses accumulator also bounds retained output to 8 MiB and 1024
  items; exceeding a bound fails explicitly.
- Content-complete request history now maps message and parallel function
  call/result items to Chat messages. `call_id`, names, raw argument strings,
  result order and schemas are retained. Orphan/duplicate/missing outputs and
  a new call batch before all prior outputs refuse. Responses item `id` and
  `status` are provenance fields omitted from Chat; they are not the tool-link
  identifier, which remains `call_id`.
- User image URL parts remain structured; developer/system roles, named tool
  choice, parallel tools, JSON Schema, reasoning effort and stateless metadata
  map explicitly. Streamed Chat targets request usage. Invalid types and fields
  without a declared mapping refuse instead of being silently ignored.
- A non-null `previous_response_id`, stored Responses, item references, opaque
  state and unsupported native content still require a Responses-native
  upstream. The public Responses route, native paths, complete capability
  validation, authorization and metering remain unqualified.
- JSON and SSE forwarding select the upstream path from the current attempt's
  translation direction, retain the caller query and configured backend base
  path, and recompute after fallback. The original caller URI remains the
  request identity; ordinary/native paths remain unchanged.
