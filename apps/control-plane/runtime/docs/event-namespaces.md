# Runtime event namespaces (P2.6)

| Namespace | Purpose |
| --- | --- |
| `aster.product.*` | Commodity invoke / usage facts for Runtime |
| `aster.platform.*` | Internal microservice bus (auth/billing/config/cloud outbox) |

Runtime must not subscribe to `aster.platform.*` for sku routing. Platform services must not publish product invoke payloads onto platform topics.
