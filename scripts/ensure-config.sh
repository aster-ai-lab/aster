#!/usr/bin/env bash
# Ensure the independent aster-config process is listening on :7652.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/config/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/config.pid"
log_file="$state_dir/config.log"
listen="${ASTER_CONFIG_LISTEN:-127.0.0.1:7652}"
port="${listen##*:}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/live" >/dev/null 2>&1 \
    && (curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1 \
      || curl -sf --max-time 2 "http://127.0.0.1:${port}/api/health" >/dev/null 2>&1)
}

bin="target/debug/aster-config"
require_repo_script "$repo_root/scripts/compose-postgres-host.sh"

if health_ok; then
  service_pid_alive || remember_listener "$port"
  if ! service_outdated "$bin" apps/config; then
    echo "aster-config already healthy on :${port}"
    exit 0
  fi
  echo "aster-config on :${port} runs an outdated build — compiling the replacement before stopping"
fi

if binary_is_stale "$bin" apps/control-plane/config; then
  echo "building aster-config…"
  CARGO_TARGET_DIR="$repo_root/target" \
    cargo build --locked --manifest-path apps/control-plane/Cargo.toml -p aster-config --bin aster-config
fi

if [[ ! -x "$bin" ]]; then
  echo "aster-config binary missing — leaving the current listener in place" >&2
  exit 1
fi

# Helper and binary succeeded. Only now replace the listener.
pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "stopping aster-config on :${port}"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting aster-config on ${listen}…"
ASTER_CONFIG_LISTEN="$listen" \
  ASTER_CONFIG_DATABASE_URL="${ASTER_CONFIG_DATABASE_URL:-postgres://aster_config:aster_config@${pg_host}:5432/aster_config}" \
  ASTER_CONFIG_IGGY_ADDRESS="${ASTER_CONFIG_IGGY_ADDRESS:-${IGGY_ADDRESS:-127.0.0.1:5102}}" \
  ASTER_CONFIG_IGGY_USERNAME="${ASTER_CONFIG_IGGY_USERNAME:-${IGGY_ROOT_USERNAME:-iggy}}" \
  ASTER_CONFIG_IGGY_PASSWORD="${ASTER_CONFIG_IGGY_PASSWORD:-${IGGY_ROOT_PASSWORD:-iggy}}" \
  ASTER_ETCD_ENDPOINTS="${ASTER_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  ASTER_SERVICE_NAME=aster-config \
  ASTER_SERVICE_HOST=127.0.0.1 \
  ASTER_SERVICE_PORT="$port" \
  launch_service "$bin"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "aster-config ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "aster-config exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "aster-config failed to become healthy — see $log_file" >&2
exit 1
