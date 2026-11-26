# First-Principles Token Optimization (Zero Capability Loss)

Date: 2026-11-26
Platform: macOS `arm64`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`

## First-Principles Summary

Total multi-turn token cost follows:

$$\text{Total Cost} = \sum_t \big(\text{Uncached Input}_t + \alpha \cdot \text{Cached Input}_t + \beta \cdot \text{Output}_t\big)$$

Across seven optimization rounds, changes targeted the dominant cost terms—**KV-cache prefix invalidation** ($\text{Uncached Input}_t$), **unnecessary auxiliary round-trips and reasoning output** ($\beta \cdot \text{Output}_t + \text{Input}_t$ on internal routing/extraction/compaction), and **redundant standing-prompt/schema/receipt wire payloads**—while preserving 100% of tool/model capabilities, JSON Schema validation (`Tool::parameters()`), behavioral contracts, UI events, and fact-log provenance:

### Round 1 Optimizations

1. **KV-Cache Prefix Purity (`fact_control.rs`, `llm/anthropic.rs`, `prompts.rs`)**
   - **Pinned path-instruction anchors (`crates/code/core/src/fact_control.rs`)**: Replaced tail-pushing `user\n<path_instructions>` onto `folded` every turn with `PathInstructionAnchors` + `inject_pinned_path_instructions`, pinning each injected digest immediately after its triggering `ToolResult` (`tool_use_id`) and after `apply_verification_context`.
   - **Anthropic 4th ephemeral cache breakpoint (`crates/code/core/src/llm/anthropic.rs`)**: Placed ephemeral `cache_control` markers on both the penultimate and final messages alongside `system` and the final tool (`4` breakpoints total) so transient tail changes keep an exact prefix cache hit on the prior message boundary.
   - **Compact turn output-language override (`crates/code/core/src/prompts.rs`)**: Tightened `turn_output_language_override("zh-CN")` from `656 B` to `386 B` (`-270 B`, `-41.2%`).

2. **Prior-Turn Reasoning Cleanup on OpenAI Wire (`crates/code/core/src/llm/openai.rs`)**
   - `OpenAiClient::convert_messages` retains full `reasoning_content` for assistant messages in the active user turn (preserving interleaved thinking across tool loops) while emitting `"reasoning_content": ""` on completed prior user turns.

3. **Non-Functional Schema `examples` Stripping (`crates/code/core/src/llm/`)**
   - Added `wire_tool_parameters` in `crates/code/core/src/llm/types.rs` and wired it into `OpenAiClient::convert_tools`, `AnthropicClient::build_request`, `GeminiClient::request`, `codex_login::convert_tools`, and `estimate_prompt_tokens`, removing JSON Schema `"examples"` arrays on the wire without altering `Tool::parameters()` validation.

4. **Standing System Prompt Cross-File Deduplication (`crates/code/core/prompts/common/`)**
   - Deduplicated overlapping prose across `system_default.md`, `system_default_response_format.md`, `runtime_contract.md`, `repository_tool_contract.md`, `repository_tool_contract_readonly.md`, and `boundaries.md` while keeping all `prompts::tests` contracts intact (`11,973 B` -> `8,286 B`).

5. **Lossless Repeated Large Tool-Output Compaction (`crates/code/core/src/fact_control/`)**
   - Added `RepeatObservation` / `observe_repeated_result` in `history_seed.rs` wired through `executor_tools.rs` and `compaction.rs`: non-JSON byte-identical repeated tool outputs `> 512 B` preserve full `AgentEvent::ToolEnd` output and `observation_digest` while collapsing model-facing history to a back-reference to `first_call_id` (resetting on `SessionCompact::compact_with_tail`).

### Round 2 Optimizations

6. **Dynamic Recalled Memory Isolation Out of `system` (`crates/code/core/src/agent/turn_context.rs`, `agent/tests.rs`)**
   - Updated `prepare_turn_context` to split `ContextType::Memory` items out of `augmented_system` into `effective_prompt`, keeping `system` byte-identical across memory recall turns so recalled memories do not bust the `system + tools` KV-cache prefix.

7. **Output-Language Override Redundancy Suppression (`crates/code/core/src/prompts.rs`)**
   - Updated `apply_turn_output_language` to skip appending `turn_output_language_override` when `existing` already embeds `output_language_contract(&language)` without a conflicting `(this turn)` override.

8. **Memory Extraction Prompt Compression (`crates/code/core/src/agent/memory_extraction_runtime.rs`)**
   - Tightened `MEMORY_EXTRACTION_SYSTEM` and `build_extraction_prompt`, omitted empty `Related existing memories:` (`None`) and empty `Compressed turn transcript:` sections, and added `turn_transcript_for_extraction` to avoid duplicating single-turn plain prompt/response pairs.

9. **Recursive Schema `examples` Stripping & Built-In Tool Definition Trimming (`crates/code/core/src/llm/types.rs`, `tools/builtin/*.rs`)**
   - Extended `wire_tool_parameters` recursively to strip nested `"examples"` keys at any schema depth, and tightened redundant prose across 9 builtin tools (`web_search.rs`, `web_fetch.rs`, `download/mod.rs`, `patch.rs`, `read.rs`, `ls.rs`, `write.rs`, `ask_user_form.rs`, `bash.rs`).

10. **Zero-Copy Terminal Escape & Progress-Frame Denoising (`crates/code/core/src/tools/builtin/bash.rs`)**
    - Added `clean_terminal_output` (`Cow::Borrowed` fast path when output contains neither ESC `0x1b` nor bare `\r`), stripping ANSI CSI/OSC escape sequences and collapsing `\r` progress-bar overwrites while preserving `\r\n` CRLF verbatim before `output_streams::capture`.

11. **Anthropic Cache Breakpoint Guard Hardening (`crates/code/core/src/llm/anthropic.rs`)**
    - Hardened `AnthropicClient::mark_message_cache_breakpoint` (with `is_cacheable_block`) so empty string content is not wrapped with `cache_control` and array content skips trailing `"thinking"`, `"redacted_thinking"`, and empty `"text"` blocks in favor of the last cacheable content block.

### Round 3 Optimizations (Telemetry-Driven Auxiliary Call & Full-Session Wire Reduction)

12. **Minimal-Reasoning Derivation for Lightweight Auxiliary LLM Calls (`config/reasoning.rs`, `llm/`, `agent/`, `agent_api/`)**
    - Live Round 2 `UsageUpdate` telemetry showed internal `pre_analysis` (`callSequence=1`) inheriting the user's `reasoning_effort = "max"` and burning `1,513–1,982` output tokens (`~7.5x` more output tokens than the real agent loop).
    - Added `ReasoningCapabilities::lowest_effort()` (`None < Minimal < Low < Medium`, e.g., `deepseek-flash -> "none"`, `k3.1-pro -> "low"`, `glm-5.3 -> "low"`, `claude-sonnet-4-6 -> "low"`, `gemini-3-flash-preview -> "minimal"`) and `LlmClient::with_minimal_reasoning()` implemented across `OpenAiClient`, `AnthropicClient` (also clearing `thinking_budget`), `ZhipuClient`, `GeminiClient`, and `LlmInvoker`.
    - Wired `with_minimal_reasoning()` exclusively into lightweight internal auxiliary calls (`run_pre_analysis`, `run_intent_classification`, `complete_memory_extraction`, `generate_session_title`, `generate_follow_up_suggestions`) while keeping primary turns, `pre_analyze_goal`, and user-invoked `generate_object` at the session's configured reasoning effort.
    - Updated `model_discovery_tests.rs` (`selected_catalog_updates_the_menu_and_execution`, `switches_rebuild_native_controls`) to separate primary-turn requests from auxiliary `You are a pre-analysis assistant.` requests and verify both independently.

13. **Intent-Only Pre-Analysis for `PlanningMode::Disabled` & Compact Fallback Schemas (`planning/llm_planner.rs`, `agent/`, `prompts/`, `llm/structured.rs`)**
    - Added `LlmPlanner::classify_intent` + `TurnEngine::run_intent_classification`, used in `fact_publish_plan` when `PlanningMode::Disabled` and `read_only_synthesis_rounds.is_some()` instead of requesting 5-field planning JSON and discarding 4 of the 5 fields (`PRE_ANALYSIS_SYSTEM` header retained for mock/hook compatibility).
    - Removed the duplicate 22-line pretty-printed `Schema: { ... }` block from `crates/code/core/prompts/analysis/pre_analysis_system.md` while keeping `"You are a pre-analysis assistant."` and all classification rules intact.
    - Switched `structured::build_initial_messages` Prompt/Json fallback schema serialization from `serde_json::to_string_pretty` to compact `serde_json::to_string`.

14. **Full-Session Tool Catalog Trimming (`tools/program_tool.rs`, `output_check.rs`, `tools/builtin/*.rs`, `tools/{skill,task}.rs`, `subagent.rs`)**
    - Stopped `ProgramTool::from_catalog` from dumping raw `serde_json::json!(template.parameters)` JSON into `description` alongside `parameters()["oneOf"]` (retaining template names, descriptions, and concise parameter names).
    - Tightened descriptions/parameter prose while preserving all JSON Schema constraints and test-asserted substrings across `OutputCheckTool`, `GenerateObjectTool`, `BackgroundTool`, `BatchTool`, `SearchTool`, `UpdatePlanTool`, `CodeSymbolsTool`, `CodeNavigationTool`, `CodeDiagnosticsTool`, `SkillTool`, `TaskTool`, and `builtin_agents()`.

### Round 4 Optimizations (Default ACP Intent Routing, Prompt Split & `oneOf` Wire Separation)

15. **Fix Default ACP `PlanningMode::Auto` Inheritance (`crates/code/acp/src/agent/session_open.rs`, `agent_tests.rs`)**
    - Set `.with_planning_mode(aster_core::PlanningMode::Disabled)` as the default in `core_session_options` while preserving `.with_planning_mode(aster_core::PlanningMode::Auto)` for `ultracode`.

16. **Split `PRE_ANALYSIS_SYSTEM` Into Compact Intent Core + Planning Addendum (`prompts/`, `planning/llm_planner.rs`)**
    - Reduced `crates/code/core/prompts/analysis/pre_analysis_system.md` to `480 B` (intent classification rules only, retaining `"You are a pre-analysis assistant."` for hook/mock compatibility).
    - Moved full multi-field planning rules (`requires_planning`, tool proposals, `complexity`, `optimized_input`, goal criteria) into `crates/code/core/prompts/analysis/pre_analysis_planning_addendum.md` (`PRE_ANALYSIS_PLANNING_ADDENDUM`), appended only by `LlmPlanner::pre_analyze`.
    - Simplified `LlmPlanner::classify_intent` user prompt to `"User request:\n{prompt}"`.

17. **Separate Wire `Tool::definition()` Without `oneOf` & Strip Empty `"required": []` (`output_check.rs`, `program_tool.rs`, `llm/types.rs`)**
    - Overrode `Tool::definition()` on `OutputCheckTool` and `ProgramTool` to omit redundant top-level `"oneOf"` unions on the LLM wire while keeping full strict `"oneOf"` schemas in `Tool::parameters()` for local validation/introspection.
    - Extended `wire_tool_parameters` in `crates/code/core/src/llm/types.rs` to recursively omit empty `"required": []` arrays on the wire while keeping non-empty `"required"` arrays intact.

18. **Further Built-In Schema/Header Trimming & Compaction Reasoning (`tools/builtin/*.rs`, `tools/task*`, `skills_catalog_header.md`, `fact_control/compaction.rs`)**
    - Tightened duplicate/verbose parameter descriptions across core built-in tools and `skills_catalog_header.md`.
    - Wired `client.with_minimal_reasoning()` into `SessionCompact::compact_with_tail` (`crates/code/core/src/fact_control/compaction.rs`) before `compact_handoff`.

### Round 5 Optimizations (Deep Core Standing-Prompt Compression, ACP Terminal Presentation, Compaction & Agent Prompts)

19. **Deep Cross-File Compression of Core Standing & Compaction Prompts (`crates/code/core/prompts/common/`)**
    - Tightened `system_default_role_line.md`, `system_default_response_format.md`, `system_default.md`, `runtime_contract.md`, `repository_tool_contract.md`, `repository_tool_contract_readonly.md`, `boundaries.md`, and `context_compact.md` while preserving all `SystemPromptSlots` and `repository_tool_parameter_schemas()` contracts (`8,286 B` -> **`5,658 B`**, cumulative **`-52.7%`**).

20. **ACP `TERMINAL_MARKDOWN_PRESENTATION`, Subagent Prompts & Tool `description()` Compression (`acp/src/agent.rs`, `acp/src/output_check.rs`, `tools/`, `prompts/agents/`)**
    - Tightened `TERMINAL_MARKDOWN_PRESENTATION` (`~980 B` -> `~500 B`), `OutputCheckTool`, built-in tool descriptions, and `prompts/agents/{explore,plan,code_review,verification}.md`.

### Round 6 Optimizations (Lazy Intent Classification, Deep Wire Schema Minification, Deprecated Alias Removal & Structured/Verification Prompt Convergence)

21. **Lazy Read-Only Synthesis Intent Classification (`loop_checkpoint.rs`, `agent/{execution_state,loop_builder,planning_runtime,execution_mode}.rs`, `fact_control.rs`)**
    - Deferred `LlmPlanner::classify_intent` under `PlanningMode::Disabled` (when `prompt_slots.style` is `None` and `read_only_synthesis_rounds.is_some()`) via `LoopConvergenceState::read_only_synthesis_classification_pending` + `fact_defer_read_only_synthesis` until `fact_observe_read_only_synthesis_tools` actually reaches the `read_only_synthesis_rounds` threshold (`fact_control.rs` resolves `fact_resolve_lazy_read_only_synthesis` right before `fact_read_only_synthesis_directive`).
    - Zero capability loss (still invokes `LlmPlanner::classify_intent` per Constitution XV whenever a session hits the exploration budget) while eliminating **100%** of auxiliary classification calls on 0-round and 1-round default ACP turns (`disabled_planning_defers_intent_classification_until_synthesis_budget` in `read_only_delivery.rs`).

22. **Deep Structural Wire Minification (`crates/code/core/src/llm/types.rs`, `llm/gemini/request.rs`)**
    - Extended `wire_tool_parameters` (`wire_tool_parameters_with_closed_object`) to recursively omit non-semantic, locally-enforced JSON Schema boilerplate across OpenAI/Anthropic/Codex wire requests: `"additionalProperties": false` (preserving map-valued `additionalProperties`), trivial `"minLength": 1` / `"minItems": 1` (keeping `>= 2`), anti-DoS `"maxLength" >= 128` (keeping small bounds like `10`/`40`), and overflow-guard `"maximum" >= 1_000_000`.
    - Added `wire_tool_parameters_preserving_closed_objects` for `GeminiClient::request` (`parametersJsonSchema`) so Gemini retains native closed-object enforcement while stripping `minLength`/`minItems`/high DoS ceilings.
    - `Tool::parameters()` stays 100% strict for local `jsonschema` validation, and `ResponseFormat::JsonSchema` (`strict: true`) is untouched.

23. **`GitTool::definition()` Alias Separation, Schema Deduplication & ACP Retrieval Tool Trimming (`tools/builtin/*.rs`, `tools/task*`, `subagent.rs`, `acp/src/{memory_search,wiki_search}.rs`)**
    - Overrode `GitTool::definition()` to omit deprecated `"max_count"` from the LLM wire while keeping `"max_count"` accepted in `GitTool::parameters()` and `execute()` for backward compatibility (`git_definition_omits_deprecated_max_count_while_parameters_keeps_it`).
    - Deduplicated schema descriptions against `default` / `enum` / `minimum` / `maximum` fields across `task.rs`, `task/parallel_params.rs`, `subagent.rs` (retaining `"well-scoped question"` in `explore`), `edit.rs`, `read.rs`, `background.rs`, `bash.rs`, `download/mod.rs`, `generate_object.rs`, `web_fetch.rs`, `web_search.rs`, `write.rs`, `ls.rs`, `update_plan.rs`, `acp/src/memory_search.rs`, and `acp/src/wiki_search.rs`.

24. **Structured Output System Framing & `[host verification state]` Convergence (`llm/structured.rs`, `harness_loop.rs`, `fact_control/tool_results.rs`, `fact_control_tests.rs`)**
    - Tightened `structured::build_system_prompt` for `StructuredMode::Tool` / `Prompt` / `Json` while keeping `build_initial_messages` `"You MUST respond with ONLY"` intact.
    - Tightened `model_verification_context` and `apply_verification_context` `[host verification state]` directives while preserving every verification and non-waiver invariant.
    - Tightened `token_optimization_measurements_report` guards (`wire_schema_only_bytes < 8_500`, `full_session_wire_bytes < 17_200`).

### Round 7 Optimizations (Cross-Turn Repeat Compaction, `LsTool` Whitespace Removal, Standing Prompt & ACP/Studio Compression)

25. **Cross-Turn Lossless Duplicate Tool-Output Compaction (`fact_control/history_seed.rs`, `fact_control.rs`, `fact_control_tests.rs`)**
    - Extracted `repeat_call_digest`, `repeat_result_digest`, and `is_compacted_repeat_stub` in `history_seed.rs` and added `seed_repeats_from_facts(&[Fact])`, replaying uncompacted `model.turn` / `tool.result` pairs after the latest matching `compaction.done` (ignoring JSON results and preserving original `first_call_id` across earlier compacted stubs).
    - Wired `seed_repeats_from_facts(&existing_facts)` into `FactRun::open_internal` (`fact_control.rs`) instead of an empty per-turn `HashMap`, so Turn 2+ re-reads of unchanged large (`> 512 B`) non-JSON outputs collapse to back-references referencing the live earlier `first_call_id` without altering prior KV-cached turns, while `SessionCompact` clears `repeats` on compaction (`cross_turn_repeat_seeding_compacts_unchanged_large_reread_and_resets_after_compaction`).

26. **Runtime `LsTool` Column-Padding Removal (`crates/code/core/src/tools/builtin/ls.rs`)**
    - Replaced `"{:<6} {:>8}  {}{}\n"` with `"{} {} {}{}\n"` and trimmed double-newline header/footer separators to single newlines, removing 8–11 pure padding bytes per entry while preserving `kind`, formatted size, name, `/` directory suffix, pagination cursor, and `metadata.page`.

27. **Core Standing Prompt & `program`/`batch`/`search` Schema Compression (`prompts/common/*.md`, `program.rs`, `tools/{program_tool,builtin/batch,builtin/search}.rs`, `fact_control_tests.rs`)**
    - Tightened `system_default_role_line.md` (kept byte-identical with the `system_default.md` opening line for `SystemPromptSlots` role replacement), `system_default_response_format.md` (synced with `system_default.md`), `system_default.md`, `runtime_contract.md`, `repository_tool_contract.md`, `repository_tool_contract_readonly.md`, and `boundaries.md`, reducing `standing_bytes` from `5,658 B` to **`5,247 B`** (`-411 B` in Round 7; **`-56.2%`** cumulative vs `11,973 B`).
    - Tightened `program_code_search` / `program_repo_map` template prose (`program.rs`), `ProgramTool::from_catalog` / `base_parameters` (`program_tool.rs`), `BatchTool::parameters` (retaining `"$ref"` out of JSON schema), and `SearchTool::parameters` (`search.rs`), cutting `full_session_wire_bytes` from `16,897 B` to **`16,458 B`** (`-439 B` in Round 7) and tightening `token_optimization_measurements_report` thresholds (`standing.len() < 5_350`, `wire_schema_only_bytes < 8_150`, `full_session_wire_bytes < 16_600`).

28. **ACP Turn Boundary & Studio Tool/Prompt Compression (`acp/src/{agent,office_file,office_file_tests,office_read,document_read,workspace_browser,host_context,output_check}.rs`, `acp/prompts/{office,browser}.md`)**
    - Tightened `SESSION_OUTCOME_BOUNDARY` in `acp/src/agent.rs` (`774 B -> 596 B`) while keeping every contract phrase asserted in `agent_tests.rs` and `prompt_boundary_tests.rs`.
    - Overrode `OfficeFileTool::definition()` via `office_file_properties(false)` to omit `blocks.items.allOf` (`if`/`then`/`not`) on the LLM wire while keeping full `allOf` validation inside `OfficeFileTool::parameters()` (`office_file_definition_omits_block_all_of_while_parameters_keeps_it`), and tightened descriptions/schemas across `OfficeFileTool`, `OfficeReadTool`, `DocumentReadTool` (preserving all 8 asserted substrings in `document_read_tests.rs`), `OpenWorkspaceBrowserTool`, `OutputCheckTool`, `USER_BROWSER_PANE` (`host_context.rs`), `office.md`, and `browser.md`.

## Observed Before / After Static Wire Measurements

| Surface | Initial Baseline | Round 1–5 | Round 6 | Round 7 (Final) | Total Delta |
|---|---:|---:|---:|---:|---:|
| `SystemPromptSlots::default().build()` (`standing_bytes`) | `11,973 B` | `5,658 B` | `5,658 B` | **`5,247 B`** | **`-6,726 B` (`-56.2%`)** |
| `turn_output_language_override("zh-CN")` (`lang_override_bytes`) | `656 B` | `386 B` | `386 B` | `384 B` | `-272 B` (`-41.5%`) |
| `PRE_ANALYSIS_SYSTEM` (`pre_analysis_sys_bytes`) | — | `480 B` (full `1,579 B`) | `480 B` (full `1,579 B`) | `480 B` (full `1,579 B`) | `-1,216 B` (`-71.7%` vs R3; lazy on `Disabled`) |
| 14 builtin `ToolDefinition` structs (`raw_defs_bytes`) | `22,263 B` | `15,038 B` | `14,146 B` | **`14,081 B`** | **`-8,182 B` (`-36.8%`)** |
| 14 builtin parameter schemas before strip (`raw_schema_bytes`) | `17,095 B` | `11,896 B` | `11,112 B` | **`11,047 B`** | **`-6,048 B` (`-35.4%`)** |
| 14 builtin parameter schemas on wire (`wire_schema_bytes`) | `17,095 B` | `9,618 B` | `8,176 B` | **`8,111 B`** | **`-8,984 B` (`-52.6%`)** |
| OpenAI `tools` JSON on wire for 14 builtins (`openai_wire_tools_bytes`) | — | `13,209 B` | `11,659 B` | **`11,594 B`** | **`-8,854 B` vs R1 (`-43.3%`)** |
| Full session tool catalog (`full_session_raw_bytes` -> `full_session_wire_bytes`) | — | `21,090 B -> 19,014 B` | `20,060 B -> 16,897 B` | **`19,621 B -> 16,458 B`** | **`-6,614 B` on wire vs R3 (`-28.7%`)** |

Printed by `token_optimization_measurements_report` after final edits:
`TOKEN_OPT_METRICS standing_bytes=5247 lang_override_bytes=384 pre_analysis_sys_bytes=480 pre_analysis_full_bytes=1579 tool_count=14 raw_defs_bytes=14081 raw_schema_bytes=11047 wire_schema_bytes=8111 schema_saved_bytes=2936 openai_wire_tools_bytes=11594 full_session_raw_bytes=19621 full_session_wire_bytes=16458`

## Real-LLM End-to-End Verification (`../config.acl`)

Executed against freshly built `target/debug/aster-acp` with `ASTER_CONFIG_FILE="$PWD/../config.acl"` in `crates/code/acp/tests/live_interactive_dialogue_real_llm.rs` (zero credentials/API keys logged):

| Model & Test | Round 2 Baseline | Round 6 | Round 7 (Final) | Delta (R2 -> R7) |
|---|---|---|---|---|
| `deepseek/deepseek-flash` (`live_multi_turn_token_and_cache_efficiency_e2e`) | `25.71s` (`t1=15575ms`, `t2=7780ms`), `model_turns=4`, `sessionTotalTokens=56640`; T1 call1 `1386+1982`, T1 call2 `12300+167`, T1 call3 `12598 (cacheRead=12416, 98.6%)+118`; T2 call1 `1371+665`, T2 call2 `12604+39`, T2 call3 `12685+29` | `10.91s`, `sessionTotalTokens=38300`, 4 calls total: T1 `8945+123`, `9193 (97.5%)+293`; T2 `9376 (96.9%)+73`, `9491 (98.5%)+96` | **PASSED (`10.58s`)**: `t1=3886ms`, `t2=4283ms`, `model_turns=4`, **4 LLM calls total (0 aux calls)**: T1 call1 `8709 (cacheRead=0)+107`, T1 call2 `8947 (cacheRead=8704, 97.3%)+275`; T2 call1 `9134 (cacheRead=8832, 96.7%)+39`, T2 call2 `9215 (cacheRead=8960, 97.2%)+113`; **`sessionTotalTokens=37253`** | Latency **`-58.8%` (`2.43x` faster)**; worker call1 prompt **`-3,591` (`-29.2%`)**; aux calls **0**; Total **`-19,387` (`-34.2%`)** |
| `kimi-01/k3.1-pro` (`live_multi_turn_token_and_cache_efficiency_e2e`) | `89.26s` (`t1=57334ms`, `t2=28951ms`), `model_turns=4`, `sessionTotalTokens=49091`; T1 call1 `1392+1513`, T1 call2 `10385+300`, T1 call3 `10843+133`; T2 call1 `1376+571`, T2 call2 `10771+73`, T2 call3 `10901+71` | `30.61s`, `sessionTotalTokens=31824`, 4 calls total: T1 `7362+225`, `7745 (95.1%)+81`; T2 `7746 (92.5%)+53`, `7856 (98.6%)+49` | **PASSED (`24.83s`)**: `t1=14342ms`, `t2=7959ms`, `model_turns=4`, **4 LLM calls total (0 aux calls)**: T1 call1 `7150 (cacheRead=null)+328`, T1 call2 `7636 (cacheRead=7150, 93.6%)+155`; T2 call1 `7525 (cacheRead=6912, 91.9%)+53`, T2 call2 `7635 (cacheRead=7525, 98.6%)+36`; **`sessionTotalTokens=31225`**; zero HTTP 400; bash receipt `\x1b`-free | Latency **`-72.2%` (`3.60x` faster)**; worker call1 prompt **`-3,235` (`-31.1%`)**; aux calls **0**; Total **`-17,866` (`-36.4%`)** |
| `kimi-01/k3.1-pro` (`real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn`, E11) | `68.31s` (`elapsed_ms=62837`, `model_turns=3`), `receipts=[(8, "denied"), (11, "succeeded")]` | `37.27s` (`elapsed_ms=33732`, `model_turns=3`), `receipts=[(8, "denied"), (11, "succeeded")]` | **PASSED (`35.00s`)**: `elapsed_ms=32090`, `model_turns=3`, `deadline_exceeded=false`, `receipts=[(8, "denied"), (11, "succeeded")]` | Latency **`-48.8%`** (`elapsed_ms -48.9%`), E11 refusal self-recovery intact |
| `glm-02/glm-5.3-flash`, `glm-02/glm-5.3` (Round 2 check) | `180.16s`, `182.09s` (`0 passed; 1 failed` each) | Upstream HTTP `429 Too Many Requests` (`code: "1113"`, `[1113][余额不足或无可用资源包,请充值。]`) — not re-run | Not re-run | External account quota/balance block |

## Final Post-Edit Verification Commands and Counts (macOS `arm64`)

All checks below ran strictly **after** the final Round 7 Rust code/prompt edit to their owning crate (`aster-core` final edit preceded step 2; `aster-acp` final edit preceded step 1):

1. `cargo fmt --all -- --check`: passed (`exit 0`).
2. `cargo clippy --locked -p aster-core --all-targets --no-deps`: passed (`exit 0`, zero warnings on modified files).
3. `cargo clippy --locked -p aster-acp --all-targets --no-deps`: passed (`exit 0`, zero warnings on modified files).
4. `cargo test --locked -p aster-core --lib token_optimization_measurements_report -- --nocapture`: **1 passed; 0 failed** (`6051 filtered out`, `0.02s`).
5. `cargo test --locked -p aster-core --lib prompts:: -- --test-threads=1`: **48 passed; 0 failed** (`6004 filtered out`, `0.01s`).
6. `cargo test --locked -p aster-core --lib llm:: -- --test-threads=1`: **702 passed; 0 failed; 8 ignored** (`5342 filtered out`, `25.75s`).
7. `cargo test --locked -p aster-core --lib planning:: -- --test-threads=1`: **58 passed; 0 failed** (`5994 filtered out`, `9.39s`).
8. `cargo test --locked -p aster-core --lib agent:: -- --test-threads=1`: **504 passed; 0 failed; 3 ignored** (`5545 filtered out`, `34.71s`).
9. `cargo test --locked -p aster-core --lib read_only_delivery -- --test-threads=1`: **5 passed; 0 failed** (`6047 filtered out`, `0.79s`).
10. `cargo test --locked -p aster-core --lib background_subagent -- --test-threads=1`: **5 passed; 0 failed** (`6047 filtered out`, `3.07s`).
11. `cargo test --locked -p aster-core --lib harness_loop:: -- --test-threads=1`: **90 passed; 0 failed** (`5962 filtered out`, `0.45s`).
12. `cargo test --locked -p aster-core --lib subagent:: -- --test-threads=1`: **61 passed; 0 failed** (`5991 filtered out`, `3.60s`).
13. `cargo test --locked -p aster-core --lib fact_control -- --test-threads=1`: **278 passed; 0 failed** (`5774 filtered out`, `17.77s`).
14. `cargo test --locked -p aster-core --lib program:: -- --test-threads=1`: **26 passed; 0 failed** (`6026 filtered out`, `0.01s`).
15. `cargo test --locked -p aster-core --lib tools:: -- --test-threads=1`: **920 passed; 1 failed; 12 ignored** (`5119 filtered out`, `87.56s`) — the single failure is the pre-existing macOS baseline issue `tools::process::owned_tests::input_failure_still_stops_the_process`.
16. `cargo test --locked -p aster-acp --lib -- --test-threads=1`: **557 passed; 0 failed; 10 ignored** (`107.56s`).
17. `cargo build --locked --bin aster --bin aster-acp`: passed (`exit 0`).
18. `cargo test --locked -p aster-acp --test tui_913_hermetic -- --test-threads=1`: **11 passed; 0 failed** (`29.19s`).
19. `ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_LIVE_MODEL=deepseek/deepseek-flash cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- live_multi_turn_token_and_cache_efficiency_e2e --ignored --exact --nocapture --test-threads=1`: **1 passed; 0 failed** (`10.58s`).
20. `ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_LIVE_MODEL=kimi-01/k3.1-pro cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- live_multi_turn_token_and_cache_efficiency_e2e --ignored --exact --nocapture --test-threads=1`: **1 passed; 0 failed** (`24.83s`).
21. `ASTER_CONFIG_FILE="$PWD/../config.acl" ASTER_LIVE_MODEL=kimi-01/k3.1-pro cargo test --locked -p aster-acp --test live_interactive_dialogue_real_llm -- real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn --ignored --exact --nocapture --test-threads=1`: **1 passed; 0 failed** (`35.00s`).
22. `git diff --check`: passed (`exit 0`, re-run after updating this `OUTCOME.md`).

(Unrelated pre-existing baseline failures noted earlier and untouched: `tools::process::owned_tests::input_failure_still_stops_the_process` on macOS, `agent_api::tests::fact_log::tool_call_fidelity::diagnostic_calls_keep_literal_arguments_and_runtime_refusals_in_model_history`, and `boundary_e2e::e_bash` under `ASTER_SANDBOX_DIR=unqualified-sandbox` — those latter two were not re-run in Rounds 6–7.)

## Explicitly not claimed

- **Windows and Linux x86-64**: Not executed natively; all unit, integration, and live E2E runs executed on macOS `arm64`.
- **Live Anthropic-protocol provider (`glm-01` / `glm-02`)**: Blocked at runtime in Round 2 by upstream billing/quota HTTP `429` (`code: "1113"`), so live Anthropic-wire `cacheWriteTokens` / `cacheReadTokens` billing responses are verified via `AnthropicClient` unit/wire-serialization tests rather than live external API calls in Rounds 3–7.
- **Live `OpenAIResponses` / `Gemini` providers**: Neither is configured in `../config.acl`, so `with_minimal_reasoning()` and `wire_tool_parameters_preserving_closed_objects()` for `GeminiClient` are verified by unit/compilation checks rather than live Gemini API calls.
- **`boundary_e2e`**: Run in Round 4 (`8 passed; 1 failed` on pre-existing `e_bash` under `ASTER_SANDBOX_DIR=unqualified-sandbox`); not re-run in Rounds 6–7.
- **Pre-existing uncommitted changes**: All pre-existing working-tree modifications (`E07`/`E46`/`E54`) were preserved untouched without staging or committing.
