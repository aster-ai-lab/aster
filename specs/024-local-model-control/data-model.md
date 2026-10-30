# Data model: local model control

## Configuration

`LocalModelPolicy` references the existing catalog through `ModelReference { providerId, modelId }`. A `ModelTarget` is a tagged model or route reference. A route has a stable ID, user label, fixed/ordered strategy and ordered model candidates. Default/quick/translate are a closed set of uses, not inferred intent categories.

Core exposes the `local_models` ACL grammar for its policy contract. Studio stores its route list in the existing ACL preference `feature.model_routing.routes` and supplies the admitted typed policy to the runtime; it does not read a second routing file. Default-use selections stay in the existing device preferences. A duplicate route, unknown field, empty/fixed-multiple candidate list or ambiguous target refuses activation. Missing routes, model existence and capabilities are checked against the admitted target/catalog at the dispatch boundary.

Local provider configuration migrates from the owned `providers.json` into `providers.acl`. Its arbitrary model metadata uses the existing ACL value codec. After successful migration the original file becomes `providers.json.migrated`. A corrupt canonical file never falls back to legacy. An existing backup never gets overwritten.

Device ACL preferences own default model selections. Provider configuration no longer persists those selections. Migration initializes only absent device choices, preserving explicit clears and newer choices.

## Invocation facts

`InvocationRecord` identifies a logical `requestId` and distinct `attemptId`, source/session, provider/model/credential IDs, optional returned model, terminal attempt status, start/end/first-token timing, observed tokens and a price snapshot.

Observed input includes cache reads and cache writes. `noCacheTokens`, `cacheReadTokens` and `cacheWriteTokens` are disjoint. Missing fields remain null. `usageComplete` distinguishes final reports from cumulative lower bounds seen before a stream interruption. Final totals require both observed input/output and final usage. Partial counters remain visible in the attempt detail but do not release reservations or claim a complete cost. Invalid partitions and arithmetic overflow refuse ingestion.

Prices are per million tokens in the existing USD/CNY set, captured at call time. Optional context tiers use full input and strictly increasing thresholds. A zero-count category needs no price; a consumed but unpriced/unknown category makes the estimate unknown. Subscription reference value and cash estimate aggregate separately per currency.

A repeated identical attempt is the same fact. A changed attempt with the same identity conflicts. Failed attempts with observed usage still contribute consumption. Request count is distinct logical requests, attempt count is distinct attempts, and unpriced/unknown coverage is reported separately from known totals.

## Runtime accounting and projection

Analytical invocation snapshots do not independently decide budget admission. The local accounting owner persists reservations and settlement for the applicable budget scopes and calendar windows. A missing final report remains unresolved; it does not free assumed spend by claiming zero. All fallback attempts remain subject to the same global budget.

The concrete store stays within its owning local Rust component and tenant profile. Studio queries filtered projections; ACP transports actual invocation facts/admission. Neither a renderer cache nor a turn-total notification becomes another consumption writer.

Studio's owning SQLite table stores the admitted immutable start snapshot, dispatch marker, immutable terminal payload, nullable measured units and the adapter's upper bounds. Its migration runs through the installed `a3s-orm` migrator. Admission uses `BEGIN IMMEDIATE` to read consumption and unresolved holds and insert the new reservation atomically. Budget counters are projections of those same facts, not a second mutable ledger. Changing a policy limit therefore does not erase earlier consumption. An identical pending start or terminal replay is idempotent; changed identities/payloads conflict. Recovery after profile ownership marks pending calls interrupted: undispatched calls are provably zero, while dispatched calls retain unresolved bounds.

Daily/monthly policy windows use the UTC offset captured with the policy and half-open boundaries. Cash reservation/settlement uses integer millionths of its declared currency, rounded up per call; subscription reference prices and other currencies do not consume that cash budget. Stop mode refuses unknown bounds, unknown unreserved history and exhausted limits. Notify mode keeps accepting calls and exposes the threshold or incomplete coverage.

The existing provider key editor supplies stable key IDs, labels, order and enabled state. Native Aster persists the complete list. Governed dispatch resolves enabled keys in configured order, tries eligible pre-output fallback keys before advancing to the next model, and records each key attempt under the same logical request. An anonymous local endpoint is eligible only when no keys are configured; an explicitly configured all-disabled key set is refused. Probe/legacy credential selection retains its first-enabled contract until its call path is migrated. Disabling a key retains its secret. Legacy scalar-key edits address only the stable legacy key identity and preserve other entries. Provider summaries omit secret values; authenticated key editing retains the existing explicit detail boundary. Smart/rotate/usage/pace/speed scheduling, quota cooldown and turn affinity remain additional required state, not claims established by ordered fallback alone.

## Provider quota observations

A quota reading carries provider/credential identity, source, observation timestamp, optional reset time, units and support/freshness/error state. Remaining unknown is distinct from exhausted. These provider observations do not overwrite user-defined local budget consumption.
