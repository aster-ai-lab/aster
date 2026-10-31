# Aster SMS roadmap

| Gate | State | Evidence / acceptance |
| --- | --- | --- |
| SMSBAO-1 | Provider library implemented, 2026-10-08 | [OUTCOME](../../docs/evidence/sms/20261008-smsbao-provider/OUTCOME.md); protocol fixtures, core contracts, read-only authenticated balance; no live send or Cloud product claim |
| SMS-CONSOLE-1 | Delivered; bounded live PG/HTTP/Console gate passed (global lint/typecheck debt retained) | [OUTCOME](../../docs/evidence/sms/20261008-sms-console/OUTCOME.md); SMSBao + virtual, Cloud/PG/Secrets/HTTP/SDK/Console slice |
| SMS-SEND-SWITCH-1 | Cloud/Console explicit real-send permission; live PG/HTTP/Console gate passed | [OUTCOME](../../docs/evidence/sms/20261008-real-send-switch/OUTCOME.md); default false, same memory/live-PG admission proof |
| SMS-CONTENT-1 | Signature/template/scenario management implemented; bounded gate passed | [OUTCOME](../../docs/evidence/sms/20261008-content-management/OUTCOME.md) |
| SMS-PROVIDER-CONSOLE-2 | Open Cloud factory/schema registry and generic Console lists/dialogs; third adapter on memory/native PG; actual account/runtime not certified | [OUTCOME](../../docs/evidence/sms/20261008-content-management/OUTCOME.md) |
| SMS-1 | Planned | Registry, virtual provider, deterministic inbox/clock/scenarios and shared conformance |
| SMS-2–5 | Planned | Cloud authority, durable dispatch/receipts, further suppliers, Flow and full live certification |

The [bounded specification](../../specs/004-provider-neutral-sms/spec.md) and
[task list](../../specs/004-provider-neutral-sms/tasks.md) distinguish this
provider slice from the remaining OS service.
