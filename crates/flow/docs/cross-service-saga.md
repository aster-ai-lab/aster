# Cross-service saga via Flow (P3.2 / Wave F2)

Default long-running compensation across Boot services uses `packages/flow`.

## Outline (usage → billing)

1. api commits usage export command (local DB)
2. Outbox publishes `kense.platform.billing.ingest` (see `UsageOutboxRelay`)
3. Flow / relay step calls billing HTTP ingest
4. On denial, compensation marks export for retry (no shared DB)

Keep product Runtime invoke out of this saga path.

## Runnable example

Ingest-deny → compensate is exercised by the Flow `compensation` example
(card decline → release inventory). Run:

```bash
cargo run -p a3s-flow --example compensation
```

Expected: `status=Completed` and output `status=compensated`.
