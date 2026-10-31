#!/usr/bin/env bash
# Serve the committed signed registry/ tree as a local static HTTP source.
#
# Trust remains the bootstrap root digest (README). This script only provides
# a stable local transport so clients can use --url instead of --github.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_DIR="${ASTER_USE_REGISTRY_DIR:-${ROOT_DIR}/registry}"
STATE_DIR="${ASTER_USE_REGISTRY_STATE_DIR:-${ROOT_DIR}/.local/serve}"
HOST="${ASTER_USE_REGISTRY_HOST:-127.0.0.1}"
PORT="${ASTER_USE_REGISTRY_PORT:-4873}"
PID_FILE="${STATE_DIR}/httpd.pid"
LOG_FILE="${STATE_DIR}/httpd.log"
URL_FILE="${STATE_DIR}/base.url"

usage() {
  cat <<'EOF'
Usage: scripts/serve_local.sh <start|stop|status|url|restart>

Environment:
  ASTER_USE_REGISTRY_HOST       Bind address (default: 127.0.0.1)
  ASTER_USE_REGISTRY_PORT       Listen port (default: 4873)
  ASTER_USE_REGISTRY_STATE_DIR  PID/log directory (default: .local/serve)
  ASTER_USE_REGISTRY_DIR        Directory to serve (default: <repo>/registry)
                              Must contain metadata/root.json. Used by mock-
                              package gates; production consumers pin the
                              committed tree + bootstrap digest, not this path.

The served base URL ends with / and exposes metadata/ and targets/ directly
(matching the GitHub raw registry/ layout clients expect).
EOF
}

require_registry() {
  if [[ ! -f "${REGISTRY_DIR}/metadata/root.json" ]]; then
    echo "error: missing ${REGISTRY_DIR}/metadata/root.json" >&2
    exit 1
  fi
}

base_url() {
  printf 'http://%s:%s/' "${HOST}" "${PORT}"
}

read_pid() {
  if [[ ! -f "${PID_FILE}" ]]; then
    return 1
  fi
  local pid
  pid="$(tr -d '[:space:]' <"${PID_FILE}")"
  if [[ -z "${pid}" || ! "${pid}" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  printf '%s\n' "${pid}"
}

process_alive() {
  local pid="$1"
  kill -0 "${pid}" 2>/dev/null
}

# True only when our recorded PID is alive AND listening on HOST:PORT.
# Prevents claiming "already running" for a stale PID or a foreign listener.
our_listener_ready() {
  local pid
  pid="$(read_pid)" || return 1
  process_alive "${pid}" || return 1

  if command -v lsof >/dev/null 2>&1; then
    local listeners
    listeners="$(lsof -nP -iTCP:"${PORT}" -sTCP:LISTEN 2>/dev/null || true)"
    if [[ -z "${listeners}" ]]; then
      return 1
    fi
    if ! printf '%s\n' "${listeners}" | awk -v pid="${pid}" 'NR>1 && $2==pid { found=1 } END { exit found ? 0 : 1 }'; then
      return 1
    fi
  fi

  curl -fsS "$(base_url)metadata/root.json" >/dev/null 2>&1
}

clear_stale_state() {
  rm -f "${PID_FILE}"
}

is_running() {
  our_listener_ready
}

cmd_start() {
  require_registry
  mkdir -p "${STATE_DIR}"

  if our_listener_ready; then
    echo "already running pid=$(read_pid) url=$(base_url)"
    exit 0
  fi

  # Drop stale PID files or dead processes before binding.
  if pid="$(read_pid 2>/dev/null || true)" && [[ -n "${pid}" ]]; then
    if process_alive "${pid}"; then
      # Alive but not our listener on PORT — do not steal a foreign process;
      # only clear the PID file if it does not own the port.
      if command -v lsof >/dev/null 2>&1; then
        listeners="$(lsof -nP -iTCP:"${PORT}" -sTCP:LISTEN 2>/dev/null || true)"
        if printf '%s\n' "${listeners}" | awk -v pid="${pid}" 'NR>1 && $2==pid { found=1 } END { exit found ? 0 : 1 }'; then
          kill "${pid}" 2>/dev/null || true
        fi
      else
        kill "${pid}" 2>/dev/null || true
      fi
    fi
    clear_stale_state
  fi

  if command -v lsof >/dev/null 2>&1; then
    if lsof -nP -iTCP:"${PORT}" -sTCP:LISTEN >/dev/null 2>&1; then
      echo "error: port ${PORT} is already in use by another process" >&2
      exit 1
    fi
  fi

  if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 is required to serve the static registry tree" >&2
    exit 1
  fi

  (
    cd "${REGISTRY_DIR}"
    # Bind explicitly; directory is registry/ so /metadata and /targets map
    # at the URL root (same relative layout as the published registry/ path).
    exec python3 -m http.server "${PORT}" --bind "${HOST}"
  ) >"${LOG_FILE}" 2>&1 &
  echo $! >"${PID_FILE}"

  local url
  url="$(base_url)"
  printf '%s\n' "${url}" >"${URL_FILE}"

  local i
  for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15; do
    if our_listener_ready; then
      echo "started pid=$(read_pid) url=${url}"
      return 0
    fi
    sleep 0.2
  done

  echo "error: registry started but failed readiness probe; see ${LOG_FILE}" >&2
  cmd_stop || true
  exit 1
}

cmd_stop() {
  local pid=""
  pid="$(read_pid 2>/dev/null || true)"
  if [[ -z "${pid}" ]]; then
    echo "not running"
    return 0
  fi
  if process_alive "${pid}"; then
    kill "${pid}" 2>/dev/null || true
    local i
    for i in 1 2 3 4 5 6 7 8 9 10; do
      if ! process_alive "${pid}"; then
        break
      fi
      sleep 0.1
    done
    if process_alive "${pid}"; then
      kill -9 "${pid}" 2>/dev/null || true
    fi
  fi
  clear_stale_state
  echo "stopped"
}

cmd_status() {
  if our_listener_ready; then
    echo "running pid=$(read_pid) url=$(base_url) ready=1"
    return 0
  fi
  if pid="$(read_pid 2>/dev/null || true)" && [[ -n "${pid}" ]] && process_alive "${pid}"; then
    echo "running pid=${pid} url=$(base_url) ready=0" >&2
    return 1
  fi
  echo "not running"
  return 1
}

cmd_url() {
  printf '%s\n' "$(base_url)"
}

case "${1:-}" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  status) cmd_status ;;
  url) cmd_url ;;
  restart)
    cmd_stop
    cmd_start
    ;;
  *)
    usage
    exit 2
    ;;
esac
