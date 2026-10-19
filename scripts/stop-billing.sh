#!/usr/bin/env bash
# Stop the background a3s-billing started by ensure-billing.sh / just up.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pid_file="apps/billing/.data/billing.pid"
port="${A3S_BILLING_LISTEN:-127.0.0.1:7651}"
port="${port##*:}"

stop_pid() {
  local pid="$1"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 40); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    echo "stopped billing pid $pid"
  fi
}

if [[ -f "$pid_file" ]]; then
  old="$(cat "$pid_file" 2>/dev/null || true)"
  rm -f "$pid_file"
  if [[ -n "${old}" ]]; then
    stop_pid "$old"
  fi
fi

for pid in $(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true); do
  cmd="$(ps -p "$pid" -o comm= 2>/dev/null || true)"
  if [[ "$cmd" == *a3s-billing* ]]; then
    stop_pid "$pid"
  fi
done

echo "a3s-billing stopped"
