# ADR 0009: Preserve v4 encoding and distinguish identity proof requirements

- Status: Implemented; full product release qualification remains open
- Date: 2026-10-17
- Clarifies: ADR 0002 and ADR 0007

## Context

The locked v4 fixtures omit an absent network egress policy. Encoding that
absence as `egress_allowlist: null` changes the specification and request
digests without changing policy intent or the schema identifier. Three frozen
wire/digest tests detected this drift. Their fixtures and hashes remain intact.

The current binding projection supports an ordinary Sandbox identity chain
without a hardware quote, but `require_identity_attestation` promised the
strict attested profile. Sharing one requirement flag allowed a missing
artifact to pass that explicit requirement.

## Decision

An absent egress policy is omitted when encoding v4. An explicit empty or
nonempty allowlist remains present and digest-bound. Empty host-service
requests are likewise omitted. Deserializing an explicit null does not make
its old, noncanonical digest an alternative valid identity.

All file-store record reads validate the complete typed record before
returning it, just as receipt reads already do. A mismatched specification
digest fails closed without rewriting the record. There is no fallback digest
or automatic state migration.

`require_identity_attachment` requires the opaque attachment and exact
generation-bound provider evidence. Ordinary Sandbox bindings may have no
attestation artifact; they do not claim hardware attestation. Confidential
bindings require the artifact, including when a constructed binding is later
validated or hashed.

`require_identity_attestation` additionally requires the advertised Attestation
capability and an actual artifact in the observation, for every isolation
level. Missing support or an absent artifact is refused.

Cloud's existing evidence adapter explicitly selects the attachment profile.
Its Confidential checks remain mandatory, and a Runtime component binding
alone still does not authorize credential issuance. Trust, freshness, tenant
scope, issuer admission and workload authorization remain caller-owned.

## Upgrade boundary

Do not upgrade a live producer/provider pair over state or queued commands
whose digests were computed from the noncanonical null encoding. Drain and
retire those generations and commands with their matching previous binaries,
retain their records/evidence, then start the corrected pair and deploy new
generations. Do not edit an existing generation's digest, relabel its provider
resource or bypass journal validation. The same boundary applies to Cloud/Node
command payload digests and Box's persisted specification labels.

This change restores frozen v4 interoperability; it does not certify a rolling
upgrade across the erroneous encoding. No user service is restarted or migrated
as part of the retained isolated tests.

## Evidence

Frozen protocol and MCP fixtures remain byte-identical. Tests cover exact
digests, explicit allowlists, both consumer profiles, Confidential artifact
removal and rejection of noncanonical stored state without byte mutation.
See [retained delivery evidence](../../../../docs/evidence/desktop/20261017-runtime-contract-compatibility/OUTCOME.md).
