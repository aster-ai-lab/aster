#!/usr/bin/env bash
set -euo pipefail
export A3S_USE_BROWSER_SOCKET_DIR="${A3S_CLOUD_INTELLIGENT_UI_BROWSER_SOCKET_DIR:-/private/tmp/a3s-intelligent-ui-browser}"
mkdir -p "$A3S_USE_BROWSER_SOCKET_DIR"
if [[ "${1:-}" == use && "${2:-}" == browser ]]; then shift 2; fi
exec "${A3S_CLOUD_MDX_BROWSER_DRIVER:-${HOME}/.local/bin/a3s-use-browser-driver}" "$@"
