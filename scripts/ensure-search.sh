#!/usr/bin/env bash
# Ensure a3s-search is listening on :8090.
# Idempotent: skips when /health already answers a current build.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="crates/search/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/search.pid"
log_file="$state_dir/search.log"
port=8090
listen="0.0.0.0:${port}"
# Local deployment key. Callers send this value as the API key.
deployment_key="${A3S_SEARCH_API_KEY:-a3s-local-search}"

health_ok() {
  curl -sf --max-time 2 "http://127.0.0.1:${port}/health" >/dev/null 2>&1
}

bin="target/debug/a3s-search"
sources=(crates/search)

if health_ok; then
  service_pid_alive || remember_listener "$port"
  if ! service_outdated "$bin" "${sources[@]}"; then
    echo "a3s-search already healthy on :${port}"
    exit 0
  fi
  echo "a3s-search on :${port} runs an outdated build — compiling the replacement before stopping"
fi

if binary_is_stale "$bin" "${sources[@]}"; then
  echo "building a3s-search…"
  cargo build --locked -p a3s-search --bin a3s-search
fi

if [[ ! -x "$bin" ]]; then
  echo "a3s-search binary missing — leaving the current listener in place" >&2
  exit 1
fi

service_pid_alive || remember_listener "$port"
if service_pid_alive; then
  echo "stopping a3s-search on :${port}"
  stop_service "$port"
fi
rm -f "$pid_file"

echo "starting a3s-search on ${listen}…"
A3S_SEARCH_API_KEY="$deployment_key" launch_service "$bin" serve --listen "$listen"

for _ in $(seq 1 80); do
  if health_ok; then
    echo "a3s-search ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  if [[ -f "$pid_file" ]] && ! kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "a3s-search exited — see $log_file" >&2
    exit 1
  fi
  sleep 0.25
done

echo "a3s-search failed to become healthy — see $log_file" >&2
exit 1
