# Prompt ownership inventory

Scope: shipped assistant execution, helper workflows and host/domain projections
on the Core → ACP → TUI/Studio paths. User-authored prompts, source documents and
installed third-party skill bodies are data/contracts, not text to rewrite globally.

| Owner / source | Decision and reason | Verification |
| --- | --- | --- |
| Core `common/system_default.md` | Keep reusable coding role and actual tool strategy. Add complete-scope reuse/stop rules from Ponytail; remove browser form implementation recipe. | Core prompt assembly; source/repair live cases |
| Core `common/runtime_contract.md` | Keep host authority and effects/verification semantics. Reuse scoped authorization; preserve explicit steering; separate parent dispatch from child work. No runtime permissions changed. | Core completion, child-policy and convergence suites |
| Core `common/boundaries.md` | Retain injection, secret, sandbox and refusal boundaries unchanged. | Existing prompt and permission tests |
| Core `common/repository_tool_contract*.md` | Retain current canonical schemas, batching, continuation and guard rules. Existing pre-task edits are baseline, not attributed to this task. | Prompt assembly/schema tests |
| Core role/response format fragments | Retain literal replacement contract and language behavior. | Slots/custom-role/output-language tests |
| Core `common/skills_catalog_header.md` | Remove Studio-specific form/card workflow; retain catalog-based relevance and scoped skill loading. | Prompt/registry tests; Studio delivery guidance owns the UI contract |
| Core `common/continuation.md` | Continue from evidence without declaring every stop unfinished; permit final/report/blocked outcomes. | Continuation and completion regression tests |
| Core `common/context_compact.md`, summary prefix | Retain current summary-only, source-carry and goal/constraint restoration contract. No summarizer task execution. | Compaction and carry tests |
| Core Explore / Plan / CodeReview / Verification | Preserve specialty ceilings. Narrow exploration; direct immutable review within requested dimensions; follow dependencies only for a concrete finding uncertainty and stop after evidence and coverage are sufficient. Use relevant failure probes in host-authorized temporary space. Plan is already bounded and remains unchanged. | Specialty prompt/permission and child-policy tests; GLM child read/search anchors and no-recursion trace |
| Core pre-analysis / intent classifier | Keep public Auto and output-label/schema APIs. Empty plans for simple requests; classify requested outcome rather than words; repair stays writable. Classifier instructions/examples are English. | Prompt/planning regressions; direct live case records actual overhead |
| Core plan / goal-extract / goal-check / goal-pre-analysis | Bound ordinary plans to requested work; retain source-owned Goal acceptance stages and fail-closed evidence criteria. Do not weaken Goal gates to make conversational examples pass. | Existing planning/Goal/completion suites |
| Core `planning/plan_execute_*` | Test-only templates, retained. They are not active primary prompt layers. | `cfg(test)` registry declarations |
| Core inline host verification, memory provenance, path rules and goal feedback | Retain protocol/evidence projections. They carry concrete admitted state rather than another persona or generic process. | Fact-log, child-policy, compaction and convergence tests |
| ACP `engineering.md` | Scope to engineering; primary dispatch only when available; review children inspect directly. Preserve reuse ladder, full affected scope and checks. Increment WorkPolicy revision to 2; retain stored-session authority/conflict handling. | New boundary tests, existing policy/reopen/child tests, live review |
| ACP outcome boundary | Permit requested explanation/diagnosis/review. Distinguish command help from actually performing an operation. Remove Office/date/UI domain recipes. | Default ACP prompt and actual source-explanation case |
| ACP effort overlays | Preserve budgets/native effort routing. Remove compulsory alternatives/exhaustive checking at high tiers; keep depth proportional to concrete risk. | All effort boundary cases and native transport tests |
| ACP session capability projection | Translate the disabled-retrieval explanation and durable-memory label into English. Preserve the admitted retrieval modes and actual tool surface. | Session-setup and retrieval-seam tests |
| ACP `office.md` / `browser.md` | Project only for corresponding admitted host flags; omit for fixed source review/encyclopedia compilation. Preserve unavailable capabilities, source fidelity, real file and actual interaction evidence. | Host matrix and existing restricted-session tests |
| ACP terminal Markdown contract | Retain terminal artifact/link behavior, no false inline browser claim. No Office capability implied. | New terminal assembly test and actual TUI initialize metadata test |
| ACP unattended proof/continuation prompts | Retain grader-bound behavior unchanged. These belong to an explicitly selected unattended run, not the ordinary interactive global prompt. Original evaluations remain frozen. | Existing unattended/Goal suites; no new benchmark pass claimed |
| TUI `aster-pager/src/acp/{mod,aster_spawn}.rs` | Active Aster path initializes terminal Markdown and spawns verified ACP companion. Reuse this path; do not create another TUI system pack. | Existing capability and sidecar tests |
| Terminal compatibility agent/shell/compaction templates | Preserve upstream layout/license and compatibility behavior. They are not the Aster primary ACP prompt pack; no generated-file or foreign-agent rewrite. | Entry-point trace; compatibility-wide runtime qualification not claimed |
| Studio `agent_plane/delivery_contract.md` | Keep source/units/counts/real-file/independent output checks. Make coding/forms/prototypes conditional on actual task/capability. Instruction text is English; reply language remains user/host-controlled. | Studio host assembly and output-fidelity tests |
| Studio source-review / encyclopedia / persona / member-memory handoffs | Retain owner-specific snapshots, exact finding schema, scope and tenant/member identity. These are domain contracts, not generic coding policies. Preserve source literals. | Existing host/review/compile tests; no tenant migration |
| Studio parameter / room-memory context | English parameter heading and room-fact rule. Preserve serialized values, member and room separation, directive limits and source text. Exact source-review output markers and user isolation markers remain literal contracts. | Native parameter and memory tests |
| Studio `PromptOptimizeButton.tsx` | Apply only selected speed/altitude/depth/range methods. Preserve prompt type/authority/literals. Add Ponytail reuse, smallest complete change, essential guarantees and evidence/stop requirements without a compulsory workflow for questions. English optimizer instructions. | Renderer selected-method, stale-input and apply-confirmation tests |
| Studio `shared/ai/prompts.ts` | Replace invented biography/version/English-only/greeting template with a bounded fact-based generator. Retain translation and title helpers unchanged. | Generation contract and actual resource editor tests |
| Studio resource / quick-phrase polish | Preserve system versus user prompt type, original language and placeholder multiplicity. Add no forced template or new authority. Retain quick-phrase contracts already meeting this requirement. | Prompt polish/token/cancellation/editor tests |
| Studio aiCore context prompts | Preserve summary XML compatibility and extraction. Remove assumed incomplete work, requested reasoning transcript and permanent clarification ban. Summary stays advisory. | Actual summarizer pipeline and 168 context tests |

## Method sources

- Ponytail full: necessity → existing project path → platform/standard library →
  installed dependency → minimum complete implementation. Never omit required
  callers, error handling, security, accessibility or verification.
- Studio built-in optimizer source was inspected before editing. Its four methods
  are used as outcome/method/evidence criteria, not four compulsory personas.
- OpenAI: https://developers.openai.com/api/docs/guides/prompt-engineering
- Anthropic: https://www.anthropic.com/research/effective-context-engineering-for-ai-agents

## Compatibility

Stored WorkPolicy bytes and digest remain authoritative. New sessions select
revision 2. An explicitly conflicting rebind must still refuse; this task does
not migrate an existing session's policy silently. Public Rust/ACP schemas,
planning defaults, execution budgets and runtime permission/completion gates stay
on the existing path.

The final English delivery contract is 4257 characters after trimming. Combined
with the existing 8000 caller-character allowance and two separators, the host
payload is 12259 characters, within ACP's unchanged 12288-character bound.
Both Studio and ACP exercise the actual complete payload in their tests.

Native Studio verification exposed three ambiguous `Cow<str>` collection lookups
in Gateway. The prerequisite fix specifies `<str>` at those existing calls;
it adds no conversion or behavior. Gateway's 99 relevant tests and all-target
Clippy completed. Subsequent unrelated Studio test caller errors were corrected
by other work in the shared checkout, not attributed to this task.
