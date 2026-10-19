#!/usr/bin/env bash
# Ensure the independent a3s-auth process is listening on :7641.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/auth/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/auth.pid"
log_file="$state_dir/auth.log"
# 0.0.0.0 so the compose gateway can open this port through
# host.docker.internal. Host probes below stay on 127.0.0.1.
listen="${A3S_AUTH_LISTEN:-0.0.0.0:7641}"
port="${listen##*:}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/live" >/dev/null 2>&1 \
    && curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1
}

require_repo_script "$repo_root/scripts/compose-postgres-host.sh"

if health_ok; then
  service_pid_alive || remember_listener "$port"
  echo "a3s-auth already healthy on :${port}"
  exit 0
fi

bin="target/debug/a3s-auth"
if binary_is_stale "$bin" apps/auth; then
  echo "building a3s-auth…"
  cargo build --locked -p a3s-auth --bin a3s-auth
fi

if [[ ! -x "$bin" ]]; then
  echo "a3s-auth binary missing — leaving the current listener in place" >&2
  exit 1
fi

# Login resolves the person in a3s-api, then checks the password stored here.
# Without the directory URL this process admits nobody.
pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "a3s-auth on :${port} is up but /health failed — stopping after the replacement was built"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting a3s-auth on ${listen}…"
A3S_AUTH_LISTEN="$listen" \
  A3S_AUTH_ISSUER="${A3S_AUTH_ISSUER:-https://127.0.0.1:${port}}" \
  A3S_AUTH_DATABASE_URL="${A3S_AUTH_DATABASE_URL:-postgres://a3s_auth:a3s_auth@${pg_host}:5432/a3s_auth}" \
  A3S_AUTH_DIRECTORY_URL="${A3S_AUTH_DIRECTORY_URL:-http://127.0.0.1:3000/v1/api/admin}" \
  A3S_AUTH_INTERNAL_TOKEN="${A3S_AUTH_INTERNAL_TOKEN:-a3s-local-internal}" \
  A3S_ETCD_ENDPOINTS="${A3S_ETCD_ENDPOINTS:-127.0.0.1:2379}" \
  A3S_SERVICE_NAME=a3s-auth \
  A3S_SERVICE_HOST=127.0.0.1 \
  A3S_SERVICE_PORT="$port" \
  launch_service "$bin"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "a3s-auth ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "a3s-auth exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "a3s-auth failed to become healthy — see $log_file" >&2
exit 1
