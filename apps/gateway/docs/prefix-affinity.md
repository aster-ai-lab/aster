# Inference target prefix affinity

Enable `prefix_affinity = true` on an inference `models "alias"` block, or on
`console_catalog "published"` to apply it to that catalog's projected models.
The default is false and omits the field from serialized policy. The feature
currently applies to `/v1/chat/completions`; other endpoints retain their
existing selection behavior.

```acl
console_catalog "published" {
  api_service = "a3s-api"
  poll_interval = 30
  prefix_affinity = true
  prefix_parallelism = 2
}
```

The existing target walk still authenticates the caller, checks the model
grant, respects task-routing candidates, filters unavailable services and
protocols, and chooses the first eligible priority group. Prefix locality
narrows only that group:

1. Derive salted SHA-256 checkpoints from canonical message fields and prompt
   context such as tools. Scope them to route, environment, credential issuance
   generation, model identity, endpoint, and any explicit `prompt_cache_key`.
   Client replay annotations and sampling parameters do not affect locality.
2. Measure locally outstanding backend operations per healthy pool member.
   A recently completed prefix can prefer a target no more than one operation
   per member above the least loaded pool. Prefer the longest shared prefix,
   then lower load and recency. A match must cover at least half of the scanned
   request so a small common header cannot pin unrelated long prompts.
3. Valid upstream-reported Prism timings train bounded, expiring prefill/decode
   estimates. Compare estimated uncached input, requested output and outstanding
   work within that same load band. Declare the actual per-replica slot count
   using `prefix_parallelism` (default1, range1–1024); work in another free slot
   is not counted as a serial queue. A 5% cost tie band preserves weighted
   selection. Missing/stale measurements retain conservative load/locality
   selection. Unmeasured revision/sticky/failover pools keep their selector.
4. An owned work lease is reserved before dispatch and released at response
   completion, fallback, error or cancellation. A 2xx response creates a prefix
   hint only after a recognized finish and clean EOF; SSE also requires DONE.
   Error frames, truncated streams and oversized observations create no hint.
   Delivery remains incremental and byte-for-byte unchanged.
5. Upstream cache counts can disprove a prediction: a reported complete miss
   removes older hints for that execution signature. Successful completion
   records the new prefix. These observations do not attest KV residency or
   available blocks. Eviction, process restarts without a published epoch, or
   another proxy can make a hint inaccurate; the engine decides actual reuse.

Each request hashes at most 256 KiB of canonical JSON, with checkpoints at 4 KiB
and message boundaries, at most 128 checkpoints, and a 1 KiB minimum prefix.
The process-local index has a five-minute lifetime and an LRU ceiling of 4096
fingerprint/target entries. It stores neither prompts nor caller cache keys,
is never persisted, and survives policy refresh only as a bounded advisory
store. Exact target descriptors and opaque backend generation identities must
still match. No fingerprint becomes a telemetry label or response field.
Canonical input bytes beyond the hashing bound are counted for cost estimation,
never discarded from the upstream request or relabeled as authoritative tokens.
Performance profiles are bounded to256 and expire after five minutes; active
work reservations are capped at4096 and remain owned by their response bodies.
Observation buffers are bounded to64KiB per SSE event and256KiB per JSON body;
exceeding the bound disables observations while forwarding the original body.

Catalog model and target identities are deterministic within the exact owning
route. Re-publishing an unchanged descriptor no longer creates random IDs.
Changing the supplier, endpoint, protocol, upstream model, or alias changes the
target identity; route/model/grant authority is still evaluated on the current
snapshot. Explicitly disabling the catalog option survives subsequent polls.

`prefix_matched_bytes` in the existing attempt log is the canonical JSON prefix
length used as a routing hint. It is not a count of cached inference tokens.
`prefix_estimated_ms` is an optional scheduling estimate, never measured TTFT,
token usage, billing authority or a service-level guarantee.
Measure first meaningful SSE content and backend cache/prefill timings to
assess improvements. Eight replicas still execute an individual request on one
replica; this feature neither shards inference nor transfers KV tensors.

For maintenance, `excluded_supplier_ids = ["exact-opaque-id"]` in the catalog
block withholds those suppliers on this Gateway without editing Cloud state or
granting another target. Use an ID from the authenticated catalog; do not infer
it from a model name. Exclusions survive later catalog polls. Source-policy
changes participate in the poll digest even when catalog bytes are unchanged.
For an immediate operation, restart at zero active requests, wait for catalog
readiness and the excluded native replica to become idle before restarting it.

DeepInfer revision
[`9bd9cf2`](https://github.com/deepseek-ai/DeepInfer/tree/9bd9cf243fc53ef9afbf8f04d391d989117d6ab9)
informed the separation between execution pressure, prefill cost, and cache
locality. Its public README excludes a complete KV system and cross-replica
load balancer. This implementation imports no DeepInfer source and does not
apply its model-specific SWA suffix truncation to other models.
