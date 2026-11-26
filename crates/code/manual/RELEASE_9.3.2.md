# Core / ACP 9.3.2 and Aster CLI 0.4.2

## Core and ACP

- Tool refusal and crash recovery keep hard safety gates while allowing one
  bounded alternative turn after a typed denial (`run.tool_refusal_cycle` stops
  repeated refusals even with altered arguments). Batched `ask_user` calls park
  trailing tool dispatches until answered, unresolved side-effect latches persist
  across turns and checkpoint caches, explicit OS shell spawn failures
  (`HostShellSpawnRefused`) stay distinct from post-spawn `Unknown` execution
  uncertainty, ACP preserves exact denial reasons (`permission.denied`), and
  governed streaming enforces semantic stream progress timeouts so SSE transport
  heartbeats alone do not stall retry recovery.
- KV-cache prefix purity: pinned path instructions follow their triggering
  tool results (`PathInstructionAnchors`), recalled memories isolate out of
  `system` into the turn prompt, Anthropic requests place four cache
  breakpoints only on cacheable blocks, and prior-turn OpenAI reasoning payloads
  clear completed turns while keeping interleaved active-turn reasoning.
- Auxiliary routing and token overhead: lightweight internal LLM calls
  (pre-analysis, intent classification, memory extraction, session titles,
  follow-ups, compaction handoff) use `with_minimal_reasoning()`, default ACP
  sessions defer `classify_intent` until read-only synthesis budgets trigger,
  wire schemas omit non-semantic boilerplate (`examples`, empty `required`,
  top-level `oneOf` / `blocks.items.allOf`, deprecated `max_count`, and
  redundant bounds) while keeping strict local `Tool::parameters()` validation,
  standing/ACP/Studio prompts and schemas are compacted without losing any
  contract, `ls` drops column space padding, `bash` strips ANSI/progress-frame
  noise, and cross-turn identical large (`> 512 B`) non-JSON tool outputs
  compact to back-references while resetting cleanly on compaction.

## Aster CLI

- Ships the matched `aster 0.4.2` + `aster-acp 9.3.2` pair integrating
  `aster-core 9.3.2`, sharing `BUILD_ID`, `SHA256SUMS`, and offline Model2Vec
  `embedding/` assets.
- `aster-pager` classifies `run.tool_refusal_cycle` and `run.tool_state_cycle`
  under `ToolProgressBlocked` and displays the actionable recovery directive
  without suggesting a generic resend.

## Delivery

Core remains a local Rust SDK. Core and ACP share version `9.3.2` and source tag
`aster-acp-v9.3.2`. Aster CLI uses `0.4.2` and source tag `aster-cli-v0.4.2`,
publishing the matched `aster` + `aster-acp` pair across macOS, Linux x86_64
and arm64 (glibc 2.31 plus static `linux-musl` and `linux-musl-arm64`), optional
`kylin-x86_64` alias, and Windows.

Both Core/ACP and CLI releases use `--latest=false` (desktop retains repository
Latest). `aster-cli-stable` and the compatibility `latest.json` on the desktop
Latest release are updated after publication.
[Release governance](../../../docs/release-governance.md)
