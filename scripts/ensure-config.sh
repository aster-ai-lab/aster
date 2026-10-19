#!/usr/bin/env bash
# Ensure the independent a3s-config process is listening on :7652.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/config/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/config.pid"
log_file="$state_dir/config.log"
listen="${A3S_CONFIG_LISTEN:-127.0.0.1:7652}"
port="${listen##*:}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/live" >/dev/null 2>&1 \
    && (curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1 \
      || curl -sf --max-time 2 "http://127.0.0.1:${port}/api/health" >/dev/null 2>&1)
}

bin="target/debug/a3s-config"
require_repo_script "$repo_root/scripts/compose-postgres-host.sh"

if health_ok; then
  service_pid_alive || remember_listener "$port"
  if ! service_outdated "$bin" apps/config; then
    echo "a3s-config already healthy on :${port}"
    exit 0
  fi
  echo "a3s-config on :${port} runs an outdated build — compiling the replacement before stopping"
fi

if binary_is_stale "$bin" apps/config; then
  echo "building a3s-config…"
  cargo build --locked -p a3s-config --bin a3s-config
fi

if [[ ! -x "$bin" ]]; then
  echo "a3s-config binary missing — leaving the current listener in place" >&2
  exit 1
fi

# Helper and binary succeeded. Only now replace the listener.
pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "stopping a3s-config on :${port}"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting a3s-config on ${listen}…"
A3S_CONFIG_LISTEN="$listen" \
  A3S_CONFIG_DATABASE_URL="${A3S_CONFIG_DATABASE_URL:-postgres://a3s_config:a3s_config@${pg_host}:5432/a3s_config}" \
  A3S_CONFIG_IGGY_ADDRESS="${A3S_CONFIG_IGGY_ADDRESS:-${IGGY_ADDRESS:-127.0.0.1:5102}}" \
  A3S_CONFIG_IGGY_USERNAME="${A3S_CONFIG_IGGY_USERNAME:-${IGGY_ROOT_USERNAME:-iggy}}" \
  A3S_CONFIG_IGGY_PASSWORD="${A3S_CONFIG_IGGY_PASSWORD:-${IGGY_ROOT_PASSWORD:-iggy}}" \
  A3S_ETCD_ENDPOINTS="${A3S_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  A3S_SERVICE_NAME=a3s-config \
  A3S_SERVICE_HOST=127.0.0.1 \
  A3S_SERVICE_PORT="$port" \
  launch_service "$bin"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "a3s-config ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "a3s-config exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "a3s-config failed to become healthy — see $log_file" >&2
exit 1
