#!/usr/bin/env bash
# Publish dedicated-hardware capacity envelopes (Enterprise GA).
# Fail closed unless required env pins are set. Default duration is 7200s (2h)
# per profile — do not run this on a laptop and claim GA.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

: "${ASTER_GATEWAY_DEDICATED_RUNNER:?set to 1 on a dedicated runner}"
: "${ASTER_GATEWAY_HW_HOST:?set dedicated host name}"
: "${ASTER_GATEWAY_HW_CPU_MODEL:?set CPU model string}"
: "${ASTER_GATEWAY_HW_MEMORY_GB:?set RAM in GiB}"

if [[ "${ASTER_GATEWAY_DEDICATED_RUNNER}" != "1" ]]; then
  echo "refuse: ASTER_GATEWAY_DEDICATED_RUNNER must be 1" >&2
  exit 2
fi

BIN="${1:-./target/release/aster-gateway}"
if [[ ! -x "$BIN" && ! -f "$BIN" ]]; then
  echo "building release aster-gateway..."
  cargo build --locked --release --bin aster-gateway
  BIN=./target/release/aster-gateway
fi

DUR="${ASTER_GATEWAY_SOAK_DURATION:-7200}"
C="${ASTER_GATEWAY_SOAK_CONCURRENCY:-16}"

for profile in http-json sse-finite openai-json openai-sse; do
  python3 scripts/soak-gateway.py \
    --bin "$BIN" \
    --profile "$profile" \
    --duration "$DUR" \
    --concurrency "$C" \
    --envelope-status published
done

echo "soak-gateway published: OK (update docs/ops/capacity-envelope-draft.md and ROADMAP with these JSON files)"
