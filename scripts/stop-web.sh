#!/usr/bin/env bash
# Stop the Console dev server started by ensure-web.sh / just up.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pid_file="apps/web/.data/web.pid"

stop_pid() {
  local pid="$1"
  if kill -0 "$pid" 2>/dev/null; then
    # bun --filter leaves the dev server in the same process group.
    kill "$pid" 2>/dev/null || true
    sleep 0.3
    kill -9 "$pid" 2>/dev/null || true
    echo "stopped console pid $pid"
  fi
}

if [[ -f "$pid_file" ]]; then
  old="$(cat "$pid_file" 2>/dev/null || true)"
  rm -f "$pid_file"
  if [[ -n "${old}" ]]; then
    stop_pid "$old"
  fi
fi

for pid in $(lsof -tiTCP:3100 -sTCP:LISTEN 2>/dev/null || true); do
  cmd="$(ps -p "$pid" -o comm= 2>/dev/null || true)"
  if [[ "$cmd" == *bun* || "$cmd" == *node* || "$cmd" == *rsbuild* ]]; then
    stop_pid "$pid"
  fi
done

echo "a3s console stopped"
