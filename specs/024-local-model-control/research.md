# Research: integrated local model control

Date: 2026-10-28. The current worktree, including its pre-existing changes, is authoritative.

Scope update 2026-10-29: the user's full-replacement goal supersedes the deferrals recorded below. `full-replacement.md` fixes the expanded requirements to baseline revision `033b17bafe0ec2b4af9153b8c825b1cedcaf8555`; `reference-baseline.json` retains primary-document hashes and the exact 63-row client table. The reference sources were read, not copied into production. The original research remains historical context.

## Existing authority and reuse

| Existing code | Finding | Decision |
| --- | --- | --- |
| `crates/code/core/src/llm/types.rs` | `TokenUsage` has non-null input/output and optional cache counts; it cannot distinguish missing input from a reported zero | Preserve that public API; introduce nullable invocation observations at the provider ingestion boundary |
| `crates/code/core/src/llm/anthropic.rs` | Anthropic input excludes separately reported cache reads/writes | Declare input accounting explicitly; normalize once rather than adding cache to every provider's input |
| `crates/code/core/src/llm/openai.rs` | Chat Completions prompt tokens include cached input | Same normalized disjoint input/cache categories; no double count |
| `crates/code/core/src/harness_evidence/usage.rs` | Existing prompt/usage snapshots are context diagnostics, explicitly not billing facts | Do not turn turn/context aggregates into a financial ledger |
| `apps/studio/src/shared/data/types/aiUsageRecord.ts` | Nullable invocation fields, price snapshots, credential attribution and timing already exist | Keep renderer field semantics; adapt recorded local invocations into this contract |
| `apps/studio/src-tauri/src/providers.rs` | Legacy local provider JSON duplicates default-model selection also written to ACL device preferences | Migrate owned provider configuration to ACL; device preferences become the sole default writer |
| `apps/studio/src-tauri/src/preferences.rs` | Existing ACL value codec preserves arbitrary metadata keys and integer precision | Reuse the codec for provider migration instead of a second serializer/parser |
| `apps/studio/src-tauri/src/agent_plane/launcher.rs` | Desktop launches the actual Core runtime through ACP with generated ACL | Preserve this execution path; connect invocation facts and admission through the existing runtime/host seam |
| `crates/code/core/src/budget.rs` | Successful-call hook lacks failed attempts, reservation identity and interrupted settlement | Retain API compatibility and add the invocation/accounting contract needed by stop budgets |
| `apps/studio/src/renderer/pages/settings/UsageSettings/useUsageData.ts` | Current usage data is organization-scoped; table rows are daily aggregates | Add a local source and true invocation detail rows, reusing chart/heatmap/panel primitives |
| `apps/studio/src-tauri/src/data_sqlite.rs` | Data-page query command operates on user-selected databases | Do not use this command as an internal product ledger API |
| `crates/orm/` and `apps/studio/native/data-sqlite/` | An installed local ORM/SQLite driver already exists | Local durable storage should reuse the owning Rust driver and typed queries; no foreign runtime/database service |

## Reference boundary

The user-provided interface reference informs neutral cards, compact filters/metrics, purple/blue chart series, ranking, heatmap and request-detail density. The pinned reference inventory describes shared model entry points and observable routed calls; its revision and source hashes are retained in `reference-baseline.json`. They are design references, not evidence that Aster implements those capabilities.

## Clarification decisions

- Complete the account-free Aster desktop scope and explicitly connected clients. Managed ToYou remains isolated.
- Persist only observed model-call metadata in usage. Prompt text, response bodies and supplier secrets are outside this store.
- Preserve unknown counts/prices/timings. A price of zero is valid only when explicitly present.
- Subscription reference prices remain separate from cash estimates. Currencies remain separate.
- Defer automatic intent classification and third-party log/session import as discussed; they are not substitutes for the agreed gateway, routing, quota and usage work.

## Wire accounting checked against current primary documentation

- [OpenAI Chat Completions reference](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create): `stream_options.include_usage` adds a final whole-request usage chunk, which can be lost on interruption. Null/missing usage is not zero. The local wire meter replaces cumulative snapshots instead of summing them, preserves missing cache fields, and separates final reported usage from the stream's DONE marker.
- [Claude streaming reference](https://platform.claude.com/docs/en/build-with-claude/streaming): `message_delta` usage is cumulative and the stream ends at `message_stop`. The local wire meter merges reported counters, retaining absent fields from earlier events, and marks the snapshot final only at the terminal event. Interrupted cumulative counters remain visible without releasing a potentially larger reservation.
- Installed `aster-orm` provides SQLite `BEGIN IMMEDIATE`, typed bound queries, migration checksums and transaction rollback. The owning accounting store reuses those primitives; no database driver or ledger service was added.
