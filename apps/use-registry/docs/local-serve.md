# Local registry serve

Status: development preview. This is a **local transport** for the committed
signed tree under `registry/`. It does not change the bootstrap trust root.

## Start / stop / status

From this repository root:

```bash
chmod +x scripts/*.sh   # once
./scripts/serve_local.sh start
./scripts/serve_local.sh status
./scripts/smoke_local.sh
./scripts/consume_local.sh
./scripts/serve_local.sh stop
```

Defaults:

| Variable | Default |
| --- | --- |
| `ASTER_USE_REGISTRY_HOST` | `127.0.0.1` |
| `ASTER_USE_REGISTRY_PORT` | `4873` |
| Base URL | `http://127.0.0.1:4873/` |

State (PID, log, recorded URL) lives under `.local/serve/` (gitignored).

Requires `python3` and `curl`. The process serves only the static `registry/`
directory (metadata + targets). No Docker and no aster-box are required for this
preview transport.

`serve_local.sh start` keeps a background `python3 -m http.server` process
bound to the configured host/port, records a PID file, and refuses to return
until `metadata/root.json` is reachable. Re-running `start` is idempotent while
that PID remains healthy.

## Client binary requirement

Consuming the Registry requires the **Aster Use package manager** CLI (`registry`
and `plugin` routes), for example Use `0.3.x` built from
[aster-ai-lab/Use](https://github.com/aster-ai-lab/Use).

Homebrew `aster-use` **capability wrappers** (currently `0.1.x`: browser / box /
office routes only) answer `Unknown Use route 'registry'` and cannot consume
this tree. Point `ASTER_USE_BIN` at a package-manager build:

```bash
# from the Use checkout
cargo build -p aster-use
export ASTER_USE_BIN="$PWD/target/debug/aster-use"
```

## Point a client at the local source

Trust is still the bootstrap root digest from the README. Transport is the
local URL:

```bash
ROOT=sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172
URL="$(./scripts/serve_local.sh url)"

"$ASTER_USE_BIN" registry source add local \
  --url "${URL}" \
  --trust-root "${ROOT}" \
  --json

"$ASTER_USE_BIN" plugin plan-install aster/registry-selftest \
  --registry-name local \
  --scope-kind user \
  --scope-id user/registry-local \
  --json
```

Or run the bundled consume check (starts the server if needed, uses an
isolated `ASTER_USE_HOME`, and proves TUF refresh + catalog resolution):

```bash
export ASTER_USE_BIN=/path/to/aster-use   # package-manager binary
./scripts/consume_local.sh
```

Loopback `http://127.0.0.1/` is allowed by Use as a local transport; HTTPS is
still required for non-loopback URLs. Do not treat GitHub, localhost, or any
other URL as authority. Replace or remove the local source when you are done;
a redirect or hostname change never rotates trust.

## Stability checks

`serve_local.sh start` waits until `GET {base}metadata/root.json` succeeds.
`smoke_local.sh` verifies:

1. root / timestamp / snapshot / targets metadata are reachable
2. served `root.json` SHA-256 matches the bootstrap pin
3. the admitted smoke target downloads (`ASTER_USE_REGISTRY_SMOKE_TARGET`, default
   `aster/registry-selftest`)
4. served root matches on-disk `${ASTER_USE_REGISTRY_DIR:-registry}/metadata/root.json`

`ASTER_USE_REGISTRY_DIR` may point `serve_local.sh` at an ephemeral assembled tree
(mock multi-package gates). Changing the directory never changes trust; clients
still pin `--trust-root`.

From the monorepo root, the first-principles gate for `just up::registry` plus
mock packages (`aster/mock-alpha`, `aster/mock-beta`, `aster/mock-echo`, and
`aster/mock-compose` with Tool+MCP+OKF+Skill+UI cross-requires) is:

```bash
just test::registry
```

`consume_local.sh` additionally verifies the package-manager client can add the
source and `plugin plan-install aster/registry-selftest` against the live local
URL, and that plan provenance **equals** the served base URL and bootstrap root
pin (transport is never trusted on its own).

For a longer soak, leave the server running and re-run `./scripts/smoke_local.sh`,
`./scripts/serve_local.sh status`, or `./scripts/consume_local.sh` periodically.

Service shape and ownership: [registry-service-architecture.md](registry-service-architecture.md).

## First-principles test gate

Run the automated gate (ephemeral port + isolated state dir):

```bash
export ASTER_USE_BIN=/path/to/aster-use   # Use package-manager 0.3.x
./scripts/test_local_registry.sh
```

| Invariant | Cases |
| --- | --- |
| I1 Transport is not trust | Wrong `--trust-root` fails at `plan-install` |
| I2 Served bytes == committed tree | Smoke root/target match disk + bootstrap pin |
| I3 Ready means our listener | Start/status only after our PID owns the port |
| I4 Lifecycle is deterministic | Status before start fails; start idempotent; stop clears readiness |
| I5 Client consume uses TUF pin | `consume_local.sh` provenance matches URL + root |
| I6 Failures are loud | Smoke fails when stopped; busy foreign port refused |

`serve_local.sh` treats a process as ready only when the recorded PID is alive,
listening on the configured port (via `lsof` when available), and
`metadata/root.json` is reachable. Stale PID files and foreign listeners do not
count as a healthy local registry.
