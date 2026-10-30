# Contract: Runtime authority boundary

## Command path

Only Studio `RuntimeClient` may translate product commands into ACP requests. Only `acp.rs` may perform raw JSON-RPC transport. Other Studio modules call typed methods and must not construct wire method names.

## Fact path

Core emits the authoritative Session/Run/Goal/Plan/Permission/Review facts. ACP emits typed updates. Studio may project those updates for display but must not make a second terminal, permission or recovery decision.

## Admitted-Run driver

Interactive and background callers pass an admitted `AgentRunStream`, generation, turn permit and optional deadline/observation into the same driver. The driver alone owns event receive, cancellation observation, worker join and terminal-event presence handling. Callers retain their distinct admission policies and outcome mapping.

## Plan

Core PlanRevision is the only current-plan source. ACP `SessionUpdate::Plan` is the only new wire projection. TodoWrite rawInput may exist only in a bounded legacy-history reader until migration completes; new production events must not depend on it.

## Permission

Core decides Allow/Deny/Ask. ACP transports Ask and its answer. Studio renders Ask and returns the exact user choice. Dropping, timeout, cancellation or unknown options refuse; Studio never auto-allows from title, kind, rawInput, session ID or tenant.

## Cutover

For each domain: stop old writer, migrate/verify, activate new reader/writer, remove old authority path, then run tests. No old-store fallback, dual-write, content-similarity identity merge or permanent feature switch is allowed.

## Process isolation

A RuntimeCell is scoped to one tenant/principal/workspace configuration. Multiple cells are permitted. Sharing state, credentials, memory, schedules or event cursors across cells is forbidden unless the existing public contract explicitly admits that scope.
