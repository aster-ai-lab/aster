#!/usr/bin/env bash
# Ensure the independent a3s-billing process is listening on :7651.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/billing/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/billing.pid"
log_file="$state_dir/billing.log"
listen="${A3S_BILLING_LISTEN:-127.0.0.1:7651}"
port="${listen##*:}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/live" >/dev/null 2>&1 \
    && curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1
}

require_repo_script "$repo_root/scripts/compose-postgres-host.sh"

if health_ok; then
  service_pid_alive || remember_listener "$port"
  echo "a3s-billing already healthy on :${port}"
  exit 0
fi

bin="target/debug/a3s-billing"
if binary_is_stale "$bin" apps/billing; then
  echo "building a3s-billing…"
  cargo build --locked -p a3s-billing --bin a3s-billing
fi

if [[ ! -x "$bin" ]]; then
  echo "a3s-billing binary missing — leaving the current listener in place" >&2
  exit 1
fi

pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "a3s-billing on :${port} is up but /health failed — stopping after the replacement was built"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting a3s-billing on ${listen}…"
A3S_BILLING_LISTEN="$listen" \
  A3S_BILLING_DATABASE_URL="${A3S_BILLING_DATABASE_URL:-postgres://a3s_billing:a3s_billing@${pg_host}:5432/a3s_billing}" \
  A3S_BILLING_INTERNAL_TOKEN="${A3S_BILLING_INTERNAL_TOKEN:-a3s-local-billing}" \
  A3S_ETCD_ENDPOINTS="${A3S_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  A3S_SERVICE_NAME=a3s-billing \
  A3S_SERVICE_HOST=127.0.0.1 \
  A3S_SERVICE_PORT="$port" \
  launch_service "$bin"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "a3s-billing ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "a3s-billing exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "a3s-billing failed to become healthy — see $log_file" >&2
exit 1
