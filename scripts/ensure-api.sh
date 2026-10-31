#!/usr/bin/env bash
# Ensure the member-plane aster-api process is listening (default :3000).
#
# The five live boot dependencies (Postgres, Redis, Iggy, RustFS, config
# center) come from the root docker-compose stack plus ensure-config.sh:
#   docker compose up -d aster-postgres aster-redis aster-iggy aster-rustfs aster-etcd
#   bash scripts/ensure-shared-databases.sh
#   bash scripts/ensure-config.sh
# Every value below stays overridable through its documented environment
# variable (see apps/api/src/bootstrap/config.rs).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/api/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/api.pid"
log_file="$state_dir/api.log"
listen_host="${HOST:-127.0.0.1}"
listen_port="${PORT:-3000}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${listen_port}/health" >/dev/null 2>&1
}

if health_ok; then
  service_pid_alive || remember_listener "$listen_port"
  echo "aster-api already healthy on :${listen_port}"
  exit 0
fi

bin="apps/api/target/debug/aster-api"
if binary_is_stale "$bin" apps/api; then
  echo "building aster-api…"
  (cd apps/api && cargo build --locked --bin aster-api)
fi

if [[ ! -x "$bin" ]]; then
  echo "aster-api binary missing — leaving the current listener in place" >&2
  exit 1
fi

require_repo_script "$repo_root/scripts/compose-postgres-host.sh"
pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"

# Bootstrap seeds the platform team through the auth client; when the
# configured aster-auth is unreachable, say so before the process dies with an
# opaque storage error.
if ! curl -sf --max-time 2 \
  "${ASTER_AUTH_BASE_URL:-http://127.0.0.1:7641}/live" >/dev/null 2>&1; then
  echo "note: aster-auth is not live at ${ASTER_AUTH_BASE_URL:-http://127.0.0.1:7641};" \
    "bootstrap seeding will fail — run scripts/ensure-config.sh and scripts/ensure-auth.sh first" >&2
fi

service_pid_alive || remember_listener "$listen_port"
if service_pid_alive; then
  echo "aster-api on :${listen_port} is up but /health failed — stopping after the replacement was built"
  stop_service "$listen_port"
fi

echo "starting aster-api on ${listen_host}:${listen_port}…"
HOST="$listen_host" \
  PORT="$listen_port" \
  DATABASE_URL="${DATABASE_URL:-postgres://aster_api:aster_api@${pg_host}:5432/aster_api}" \
  REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}" \
  IGGY_ADDRESS="${IGGY_ADDRESS:-127.0.0.1:5102}" \
  IGGY_ROOT_USERNAME="${IGGY_ROOT_USERNAME:-iggy}" \
  IGGY_ROOT_PASSWORD="${IGGY_ROOT_PASSWORD:-iggy}" \
  RUSTFS_ENDPOINT="${RUSTFS_ENDPOINT:-http://127.0.0.1:9000}" \
  RUSTFS_ACCESS_KEY="${RUSTFS_ACCESS_KEY:-kense}" \
  RUSTFS_SECRET_KEY="${RUSTFS_SECRET_KEY:-aster-rustfs-dev-secret}" \
  RUSTFS_BUCKET="${RUSTFS_BUCKET:-aster}" \
  ASTER_CONFIG_URL="${ASTER_CONFIG_URL:-http://127.0.0.1:7652}" \
  ASTER_AUTH_BASE_URL="${ASTER_AUTH_BASE_URL:-http://127.0.0.1:7641}" \
  ASTER_AUTH_INTERNAL_TOKEN="${ASTER_AUTH_INTERNAL_TOKEN:-aster-local-internal}" \
  ASTER_ETCD_ENDPOINTS="${ASTER_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  ASTER_API_MAIL_SMTP_BIND="${ASTER_API_MAIL_SMTP_BIND:-127.0.0.1:2525}" \
  launch_service "$repo_root/$bin"

for _ in $(seq 1 240); do
  if health_ok; then
    echo "aster-api ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "aster-api exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.5
done

echo "aster-api failed to become healthy — see $log_file" >&2
exit 1
