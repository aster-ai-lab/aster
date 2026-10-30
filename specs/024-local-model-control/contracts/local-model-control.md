# Local model control contracts

## Ingestion

- One logical request may have multiple attempts. Attempt identity is stable across replay and cannot be reused for different content.
- Provider wire adapters declare whether input includes cache. They preserve absent usage before legacy non-null counters lose that distinction.
- Identity, usage partitions, pricing and timing are validated once at their external/durable boundary. Internal projections consume admitted typed facts.
- Usage metadata contains references and measurements, never supplier secret values or message bodies.
- Built-in source facts use the native closed set `aster-chat`, `quick-assistant`, `translate`, `model-check`, `aster-text`, `library-indexing`, mirrored by the strict IPC source field. Each caller declares its source; no prompt matching is allowed. Missing source on a legacy one-shot input means `aster-text`.

## Dispatch and admission

- Resolve explicit model/route references against the admitted catalog and the request's supported capabilities.
- Preserve candidate order. Ineligible candidates carry a reason. Unknown strategies/targets never default to another model.
- Retry/fallback is allowed only before user-visible output and within supported conversation/account state rules.
- Every attempt uses the same accounting authority. Reminder and stop policies have distinct visible behavior; unsupported stop guarantees refuse activation.
- Explicit setup probes may resolve a disabled or not-yet-saved model without enabling or rewriting it. They test only the selected credential, still pass shared budget admission and preserve observed failed consumption. An unsaved temporary credential has no fabricated key identity. Codex's auth-owner Responses probe uses the same dispatcher, budget and ledger with account attribution; its fixed SDK endpoint and auth owner remain authoritative. Real SDK/HTTP/SQLite tests cover terminal settlement, failed consumption, budget refusal and cancellation. Installed ACP turn-level coverage remains a separate T005 obligation.

## Settings and queries

- Native configuration commands use typed DTOs, registered host commands and the calling renderer's capability.
- A one-shot `requestId` opts into native cancellation through `ai_text_abort`. A duplicate active identity is refused before generation; cancellation targets only that active future, and completion/drop releases it. Missing or finished identities are idempotent. Calls without a request ID retain their existing response contract. The local accounting owner retains observed or unknown consumption when a dispatched call is cancelled; cancelling locally does not assert a vendor refund.
- Provider/model settings own configuration; usage and gateway surfaces link to those settings instead of copying editable values.
- Local usage requires no organization or account. Existing managed usage remains organization-scoped.
- Overview, trend, ranking, request list, detail and CSV use the same explicit filter generation. Currency and reference-value semantics are preserved.
- Source coverage is limited to observed Aster calls and connected clients. Missing history is never silently replaced by reconstructed turn estimates.

### Native usage reads (implemented source; focused verification pending)

- `local_model_usage_aggregate({ filter, group, utcOffsetMinutes })`: groups are `total`, `day`, `model`, `provider`, `source`. Day labels use the explicitly supplied UTC offset. Each aggregate uses one SQLite transaction for distinct request counts, attempt statuses and measured sums.
- `local_model_usage_page({ filter, cursor, limit })`: limit is 1–100. Returns `entries` and a nullable `nextCursor`; the last page has no cursor. The keyset orders by descending start time and attempt identity.
- Both commands use the profile-owned accounting state and `allow-local-model-usage`; an unavailable local host/store refuses reads. Managed hosts do not fall back to local data.
- The shared camel-case filter has `fromMs` (inclusive), `toMs` (exclusive), optional `requestId`, `sourceId`, `providerId`, `modelId`, `gatewayClientId`, and terminal `status`. Ranges are at most 366 days.
- Aggregate `knownTokens` is nullable and is accompanied by `unknownTokenAttempts`. `usdMicro` and `cnyMicro` remain separate nullable cash sums; `unknownCostAttempts` and `referencePriceAttempts` distinguish missing cash estimates from subscription reference values. Attempt status counts distinguish succeeded, failed, cancelled, interrupted and pending. These are not turn-level token snapshots.

## Gateway

- The external listener defaults to loopback and the desktop lifecycle. Listener state changes do not disable internal accounting.
- Incoming client key identity and model/route grants are separate from supplier credentials. Secrets use the existing host boundary and constant-time verification.
- Only protocol capabilities proven by the loopback tests are advertised. Unsupported protocol/shape requests refuse with a named error.
- The shared Agent-plane HTTP adapter has an explicit 16 MiB request bound so private model inference can carry long contexts; oversized bodies refuse before dispatch. One-shot text generation retains its existing 256 KiB input / 1 MiB renderer-response bounds.

## Completion evidence

The final audit maps every FR/SC/task to actual passing tests and rendered artifacts. Planning files, source presence, screenshots alone and green unrelated checks do not prove the end-to-end feature.
