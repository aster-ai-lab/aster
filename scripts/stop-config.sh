#!/usr/bin/env bash
# Stop the background a3s-config started by ensure-config.sh / just up.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

state_dir="apps/config/.data"
pid_file="$state_dir/config.pid"
port="${A3S_CONFIG_LISTEN:-127.0.0.1:7652}"
port="${port##*:}"

if [[ -f "$pid_file" ]]; then
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  if [[ -n "${pid}" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    echo "stopped config pid $pid"
  fi
  rm -f "$pid_file"
fi

# Also kill any stray config process on the default port.
if command -v lsof >/dev/null 2>&1; then
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    cmd="$(ps -p "$pid" -o args= 2>/dev/null || true)"
    if [[ "$cmd" == *a3s-config* ]]; then
      kill "$pid" 2>/dev/null || true
    fi
  done < <(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)
fi

echo "a3s-config stopped"
