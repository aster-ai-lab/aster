#!/usr/bin/env bash
# Short smoke soak for the capacity/soak harness (not a published envelope).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BIN="${1:-./target/debug/aster-gateway}"
if [[ ! -x "$BIN" && ! -f "$BIN" ]]; then
  echo "building aster-gateway..."
  cargo build --bin aster-gateway
  BIN=./target/debug/aster-gateway
fi

DUR="${ASTER_GATEWAY_SOAK_DURATION:-30}"
HTTP_C="${ASTER_GATEWAY_SOAK_CONCURRENCY_HTTP:-16}"
SSE_C="${ASTER_GATEWAY_SOAK_CONCURRENCY_SSE:-8}"

python3 scripts/soak-gateway.py --bin "$BIN" --profile http-json --duration "$DUR" --concurrency "$HTTP_C" --envelope-status smoke-only
python3 scripts/soak-gateway.py --bin "$BIN" --profile sse-finite --duration "$DUR" --concurrency "$SSE_C" --envelope-status smoke-only
python3 scripts/soak-gateway.py --bin "$BIN" --profile openai-json --duration "$DUR" --concurrency "$HTTP_C" --envelope-status smoke-only
python3 scripts/soak-gateway.py --bin "$BIN" --profile openai-sse --duration "$DUR" --concurrency "$SSE_C" --envelope-status smoke-only
echo "soak-gateway smoke: OK"
