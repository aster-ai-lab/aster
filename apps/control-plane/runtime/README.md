# aster-runtime — Gateway `/v1` data-plane (Rust)

Crate name stays `aster-runtime`. The process name is `aster-runtime`.

```text
Client  →  aster-gateway :8080/v1
        →  aster-runtime :8787
        →  Provider (by listing sku)
        →  L1 showback (GET /v1/usage)
```

**Naming clarity:** this crate is the **commodity service-invoke** data plane
behind Gateway. It is **not** Aster Agent Runtime / Box (the sandbox lifecycle
used by Cloud `AR0` / Wave H). Do not treat paths here as Agent egress,
Workspace materialization, or checkpoint APIs. See
[ax-inspired-agent-platform-plan.md](../../docs/ax-inspired-agent-platform-plan.md).

**Authoritative product docs:**

| Doc | Content |
| --- | --- |
| [SERVICE-INVOKE.md](../../apps/web/docs/console/SERVICE-INVOKE.md) | Service invoke contract |
| [SERVICE-DEV.md](../../apps/web/docs/console/SERVICE-DEV.md) | Adapter / mapping / tests |
| [WORK-CONNECT.md](../../apps/web/docs/console/WORK-CONNECT.md) | baseURL + Capability Key |

## Invoke

```text
POST /v1/services/{kind}/invoke/{sku}
```

| Segment | Meaning |
| --- | --- |
| `kind` | `model` \| `tool` \| `api` \| `feed` |
| `sku` | Opaque listing id |

| kind | Body |
| --- | --- |
| `model` | `{ "messages": [...] }` (+ optional `model` for Boyue) |
| `tool` / `api` / `feed` | `{ "input": { ... } }` — snake_case only |

Discovery: `GET /v1/services/models` (`/v1/models` stays as a direct alias; through the gateway, `/v1/models` is the OpenAI-compatible model list).

## Run

```bash
cd apps/runtime
cargo run
```

Defaults: `HOST=0.0.0.0`, `PORT=8787`. Optional: `ASTER_FUNCTION_DATA_DIR` (usage jsonl + vault receipts), provider env keys (see `.env.example`).

Demo Capability Key: `aster_sk_devlocalgateway00000000000001`

```bash
curl -sS http://127.0.0.1:8787/v1/health

curl -sS http://127.0.0.1:8787/v1/services/model/invoke/boyue.openai.chat \
  -H "Authorization: Bearer aster_sk_devlocalgateway00000000000001" \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek-v4-flash","messages":[{"role":"user","content":"ping"}]}'

curl -sS http://127.0.0.1:8787/v1/services/api/invoke/aster.monitor.chokepoint \
  -H "Authorization: Bearer aster_sk_devlocalgateway00000000000001" \
  -H "Content-Type: application/json" \
  -d '{"input":{}}'
```

Providers fail closed with `provider.credential_missing` when vault and env keys are absent.

## Local vault (loopback only)

- `POST /v1/__local/vault/materialize`
- `GET /v1/__local/vault/status`
- `GET /v1/__local/vault/receipts`
- `POST /v1/__local/queue/vault-apply` (applies in-process; NATS not wired)
- `PUT /v1/__local/upstream-secrets` (legacy plaintext patch)
