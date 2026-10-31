# Aster Config (`aster-config`)

Distributed configuration service for Aster OS.

| Layer | Role |
| --- | --- |
| **Bootstrap (env)** | Process start: `ASTER_CONFIG_DATABASE_URL`, `ASTER_CONFIG_IGGY_ADDRESS`, listen host/port. Not hot-reloadable. |
| **Runtime (KV)** | Hot keys in Postgres + in-process cache + Iggy topic `config_changed` fan-out. |

## Listen

Default `127.0.0.1:7652` (`ASTER_CONFIG_LISTEN` or `ASTER_CONFIG_HOST`/`ASTER_CONFIG_PORT`).

## HTTP

Prefix `/api/configs` (also `/health`, `/api/health`).

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `/api/configs/status` | local revision · publisher_id · change-channel name |
| `GET` | `/api/configs/bootstrap` | this process bootstrap catalog |
| `GET` | `/api/configs/builtins` | runtime builtin catalog |
| `GET` | `/api/configs/{namespace}` | list |
| `GET` | `/api/configs/{namespace}/{key}` | get |
| `PUT` | `/api/configs/{namespace}/{key}` | `{ "value": <json>, "updated_by": "..." }` |
| `DELETE` | `/api/configs/{namespace}/{key}` | delete |

### Builtin schemas

Each runtime builtin declares a `schema` (in `application/catalog.rs`), returned on every
`/builtins` item: `boolean`, `integer` (`min`/`max`/`unit`), `text`, `url`, `secret`, `enum`,
`image_data_url`, or `object` with typed `fields` (a field may be `visible_when` a sibling
boolean). The Console renders its form from it, and `PUT` on a builtin key rejects a value that
does not match with `400` naming the offending path (e.g. `value.quota is required`).

## Consumers

- **aster-api** uses `ASTER_CONFIG_URL` (`HttpConfigClient`) for `ConfigReader`, and proxies Console `/api/configs/*` to this process.
- Replicas of this service share Postgres and each poll Iggy from the head, so every replica sees a change.

## Local

```bash
bash scripts/ensure-shared-databases.sh   # creates aster_config DB
bash scripts/ensure-config.sh             # :7652
# or: cargo run --manifest-path apps/config/Cargo.toml
```

Env defaults:

```
ASTER_CONFIG_DATABASE_URL=postgres://aster_config:aster_config@127.0.0.1:5432/aster_config
ASTER_CONFIG_IGGY_ADDRESS=127.0.0.1:5102
ASTER_CONFIG_LISTEN=127.0.0.1:7652
```
