#!/usr/bin/env bash
# Ensure the independent aster-billing process is listening on :7651.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/billing/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/billing.pid"
log_file="$state_dir/billing.log"
listen="${ASTER_BILLING_LISTEN:-127.0.0.1:7651}"
port="${listen##*:}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/live" >/dev/null 2>&1 \
    && curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1
}

require_repo_script "$repo_root/scripts/compose-postgres-host.sh"

if health_ok; then
  service_pid_alive || remember_listener "$port"
  echo "aster-billing already healthy on :${port}"
  exit 0
fi

bin="target/debug/aster-billing"
if binary_is_stale "$bin" apps/control-plane/billing; then
  echo "building aster-billing…"
  CARGO_TARGET_DIR="$repo_root/target" \
    cargo build --locked --manifest-path apps/control-plane/Cargo.toml -p aster-billing --bin aster-billing
fi

if [[ ! -x "$bin" ]]; then
  echo "aster-billing binary missing — leaving the current listener in place" >&2
  exit 1
fi

pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "aster-billing on :${port} is up but /health failed — stopping after the replacement was built"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting aster-billing on ${listen}…"
ASTER_BILLING_LISTEN="$listen" \
  ASTER_BILLING_DATABASE_URL="${ASTER_BILLING_DATABASE_URL:-postgres://aster_billing:aster_billing@${pg_host}:5432/aster_billing}" \
  ASTER_BILLING_INTERNAL_TOKEN="${ASTER_BILLING_INTERNAL_TOKEN:-aster-local-billing}" \
  ASTER_ETCD_ENDPOINTS="${ASTER_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  ASTER_SERVICE_NAME=aster-billing \
  ASTER_SERVICE_HOST=127.0.0.1 \
  ASTER_SERVICE_PORT="$port" \
  launch_service "$bin"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "aster-billing ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "aster-billing exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "aster-billing failed to become healthy — see $log_file" >&2
exit 1
