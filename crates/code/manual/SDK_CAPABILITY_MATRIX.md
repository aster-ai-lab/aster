# A3S Code Rust SDK capability matrix

The Rust Core function `sdk_capabilities()` is the source of truth for the
ordered capability contract inventory. This repository delivers the local Rust
SDK and ACP; there are no foreign-language SDK artifacts or publishing jobs.

```rust
let contracts = a3s_code_core::sdk_capabilities();
let baseline = a3s_code_core::sdk_baseline_capabilities();
```

An inventory entry declares a contract. It does not assert that this build,
provider or session can execute it. `tier` distinguishes baseline from optional
host/feature surfaces; `host_owned` identifies the owner of policy, credentials
or lifecycle. Use session tool definitions/presentation, resource status and typed
refusals to determine the current executable surface.

## Contracts

Canonical operation identifiers below name Rust Core contracts, not another
SDK's method spelling. Public methods and types remain defined by
[Core](../core/src/lib.rs) and [AgentSession](../core/src/agent_api/session_facade.rs).

| ID | Tier | Canonical operations |
| --- | --- | --- |
| `agent_runtime` | baseline | `agent.create`, `agent.session`, `agent.resume_session`, `agent.close` |
| `conversation` | baseline | `session.send`, `session.run`, `session.stream`, `session.send_with_attachments`, `session.history`, `session.cancel` |
| `run_control` | baseline | `session.steer`, `session.interrupt`, `session.run_control_snapshot` |
| `governed_tools` | baseline | `session.tool`, `session.governed_tool`, `session.tool_definitions` |
| `workspace_tools` | baseline | `session.read_file`, `session.write_file`, `session.ls`, `session.edit_file`, `session.patch_file`, `session.bash`, `session.glob`, `session.grep`, `session.git`, `session.tool:download`, `session.tool:batch` |
| `workspace_retrieval` | baseline | `session.workspace_retrieval_status`, `session.semantic_search`, `session.hybrid_search` |
| `model_adapters` | baseline | `agent.create`, `session.send`, `session.stream` |
| `structured_output` | baseline | `session.send`, `session.run`, `session.task` |
| `mcp_and_skills` | baseline | `session.add_mcp`, `session.remove_mcp`, `session.mcps`, `session.add_skill`, `session.skill_names` |
| `planning_delegation` | baseline | `session.task`, `session.tasks`, `session.register_worker_agent` |
| `priority_scheduling` | baseline | `session.task_scheduler_stats`, `session.task_scheduler_health`, `session.model_generation_pool_health`, `session.model_middleware_health`, `session.queue_stats`, `session.set_lane_handler` |
| `persistence` | baseline | `session.save`, `agent.resume_session`, `session.get_artifact` |
| `governance` | baseline | `session.pending_confirmations`, `session.confirm_tool_use`, `session.register_hook`, `session.set_budget_guard`, `session.verify_commands` |
| `run_observability` | baseline | `session.runs`, `session.run_snapshot`, `session.run_events`, `session.run_event_page`, `session.active_tools`, `session.subagent_tasks` |
| `context_memory` | baseline | `session.memory`, `session.remember`, `session.recall`, `session.memory_stats` |
| `web_search` | baseline | `session.web_search`, `session.tool:web_search` |
| `web_fetch` | baseline | `session.tool:web_fetch` |
| `code_intelligence` | advanced | `session.tool:code_symbols`, `session.tool:code_navigation`, `session.tool:code_diagnostics` |
| `packages` | advanced | `session.package_binding`, `session.current_package_binding` |
| `use_runtime_tasks` | advanced | `session.tool:use_runtime_task` |
| `program` | baseline | `session.program`, `session.tool:program` |
| `programmable_workflows` | advanced | `session.parallel`, `session.parallel_resumable`, `session.workflow_step` |
| `state_graph` | advanced | `state_graph.create`, `state_graph.restore`, `state_graph.propose_patch`, `state_graph.diff` |
| `agent_release_contract` | advanced | `release.admit`, `release.bind_publication`, `release.verify` |
| `agent_protocol` | advanced | `agent_protocol.start`, `agent_protocol.cancel`, `agent_protocol.recover`, `agent_protocol.events` |
| `evaluation_substrate` | advanced | `evaluation.evidence`, `evaluation.auxiliary`, `evaluation.dispatch_ledger`, `evaluation.result`, `evaluation.result_store`, `evaluation.wire_v1` |
| `typed_decisions` | advanced | `typed_decision.decide`, `typed_decision.decide_and_gate`, `typed_decision.receipt_v1`, `typed_decision.route` |
| `moli_runtime` | advanced | `moli.default`, `moli.ensure`, `moli.packaged` |
| `s3_workspace` | advanced | `session.workspace_backend:s3`, `session.read_file`, `session.write_file` |
| `opentelemetry` | advanced | `telemetry.init`, `session.trace_events` |

## Activation and authority

- Core defaults to `local-code`. Workflows/state graphs/evaluation use explicit
  advanced features; S3, telemetry and typed decisions retain their own gates.
- Language services, packages and Use tasks require an authoritative
  host provider. A registry entry or model prompt does not supply that provider.
- Semantic retrieval requires an explicit embedding route. Workspace configuration
  cannot authorize source egress or widen inherited capabilities.
- Browser search is an explicit ACP `headless-search` feature. Ordinary API/HTTP
  search remains available without it.
- Native Office tools require explicit ACP `a3sHost.officeTools` metadata and
  remain subject to workspace permissions. Source-review restrictions dominate.
- Rust hosts can inject native `LlmClient`, `ContextProvider`, `MemoryStore`,
  `BashSandbox` and `Tool` implementations. Their owners retain credentials,
  authorization, resource bounds and teardown obligations.

## Verification

Core inventory tests guard stable IDs and canonical operations. Owning-package
integration tests verify actual session behavior; historical upstream SDK parity
reports do not qualify this product. See [Capability Verification](CAPABILITY_VERIFICATION.md),
[SDK API Design](SDK_API_DESIGN.md) and [Release Profiles](RELEASE_PROFILES.md).
