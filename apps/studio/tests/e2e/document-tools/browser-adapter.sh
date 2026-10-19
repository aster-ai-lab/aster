#!/usr/bin/env bash
set -euo pipefail

# A3S Test's `a3s` integration invokes `use browser`. An installed leaf driver
# accepts the same protocol after that prefix. Keep the override test-local.
driver="${A3S_CLOUD_DOCUMENT_TOOLS_BROWSER_DRIVER:-${HOME}/.local/bin/a3s-use-browser-driver}"
export A3S_USE_BROWSER_SOCKET_DIR="${A3S_CLOUD_DOCUMENT_TOOLS_SOCKET_DIR:-/tmp/a3s-doc-browser}"
mkdir -p "$A3S_USE_BROWSER_SOCKET_DIR"
if [[ ! -x "$driver" ]]; then
  printf 'Missing browser driver: %s\n' "$driver" >&2
  exit 1
fi
if [[ "${1:-}" == use && "${2:-}" == browser ]]; then
  shift 2
fi
exec "$driver" "$@"
