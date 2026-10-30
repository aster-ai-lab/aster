# Data model: Studio single runtime

## Ownership rule

Execution facts live only in Core. ACP carries versioned protocol representations. Studio may cache derived read models for rendering, but those caches are disposable and never used as execution or recovery authority.

## RuntimeCell

- Identity: tenant/principal/workspace plus canonical workspace and configuration generation.
- Owns: one ACP process, one Core Agent, session stores, execution scheduler and event stream for that isolation cell.
- Does not imply a global singleton process.

## ExecutionRequest

- `source`: `Prompt | GoalTick | Schedule`.
- `attendance`: `Interactive | Background`.
- `operation_id`: idempotent admission identity supplied by the caller/host.
- `permission_profile`: existing Core policy identity, never a Studio tool-name override.
- `continuation_policy`, `deadline`, `budget`: optional existing Core policy values.

Both fields with closed sets implement `parse()` / `as_str()` and reject unknown values.

## RuntimeEvent

- `origin`: `Live | Replay | Recovery | HostProjection`.
- Identities: session, runtime incarnation, prompt/run/goal/plan revision when applicable.
- Payload: typed ACP standard update or registered A3S extension.
- Unknown required events produce a protocol/degraded error, never silent `_ => {}` loss.

## Core-owned records

- Session/Transcript/Fact: stable session, message/fact and turn boundaries.
- Run: admission, cancellation, terminal outcome, usage and recovery facts.
- Goal/Plan: goal/plan revisions, sequence and work-policy digest.
- Review: target digest, finding identity and settlement state.
- ScheduleFire: schedule occurrence, operation ID and resulting Core Run identity.

## Studio presentation-only records

- Agent/workspace display metadata, pin/order/archive presentation and window bindings.
- Unsaved editor buffers and device handles.
- Disposable UI projections with source revision/cursor.

Studio presentation records must not contain authoritative message text, execution status, permission posture, runtime resume tokens, Goal state or TaskRun equivalents.

## Migration receipt

- Source/target identities and schema versions.
- Imported, unmapped, conflicting and non-executable legacy-evidence counts.
- Digest, backup location and completion marker.
- A failed or incomplete receipt blocks the new single-writer path; it never activates dual-read.
