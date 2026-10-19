#!/usr/bin/env bash
# Stop the search listener started by ensure-search.sh.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

pid_file="crates/search/.data/search.pid"

if [[ -f "$pid_file" ]]; then
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  if [[ -n "${pid}" ]] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    sleep 0.3
    kill -9 "$pid" 2>/dev/null || true
    echo "stopped a3s-search pid $pid"
  fi
  rm -f "$pid_file"
fi

if curl -sf --max-time 1 "http://127.0.0.1:8090/health" >/dev/null 2>&1; then
  for pid in $(lsof -tiTCP:8090 -sTCP:LISTEN 2>/dev/null || true); do
    kill "$pid" 2>/dev/null || true
    sleep 0.2
    kill -9 "$pid" 2>/dev/null || true
    echo "stopped listener on :8090 pid $pid"
  done
fi

echo "a3s-search stopped"
