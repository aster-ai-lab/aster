#!/usr/bin/env bash
# Smoke-check a locally served signed registry tree.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_DIR="${ASTER_USE_REGISTRY_DIR:-${ROOT_DIR}/registry}"
EXPECTED_ROOT="${ASTER_USE_REGISTRY_EXPECTED_ROOT:-sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172}"
# Relative to registry URL root. Override when smoking a mock multi-package tree.
TARGET_REL="${ASTER_USE_REGISTRY_SMOKE_TARGET:-targets/extensions/aster/registry-selftest/0.1.0/stable/any/aster-registry-selftest-0.1.0-any.tar.gz}"
BASE_URL="${1:-${ASTER_USE_REGISTRY_URL:-}}"

if [[ -z "${BASE_URL}" ]]; then
  if [[ -x "${ROOT_DIR}/scripts/serve_local.sh" ]]; then
    BASE_URL="$("${ROOT_DIR}/scripts/serve_local.sh" url)"
  else
    BASE_URL="http://127.0.0.1:4873/"
  fi
fi

# Normalize trailing slash.
case "${BASE_URL}" in
  */) ;;
  *) BASE_URL="${BASE_URL}/" ;;
esac

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

curl -fsS "${BASE_URL}metadata/root.json" -o "${tmpdir}/root.json"
curl -fsS "${BASE_URL}metadata/timestamp.json" -o "${tmpdir}/timestamp.json"
curl -fsS "${BASE_URL}metadata/snapshot.json" -o "${tmpdir}/snapshot.json"
curl -fsS "${BASE_URL}metadata/targets.json" -o "${tmpdir}/targets.json"

actual="sha256:$(shasum -a 256 "${tmpdir}/root.json" | awk '{print $1}')"
if [[ "${actual}" != "${EXPECTED_ROOT}" ]]; then
  echo "error: root digest mismatch" >&2
  echo "  expected: ${EXPECTED_ROOT}" >&2
  echo "  actual:   ${actual}" >&2
  exit 1
fi

# Confirm a known admitted target object is reachable.
curl -fsS "${BASE_URL}${TARGET_REL}" -o "${tmpdir}/pkg.tar.gz"
if [[ ! -s "${tmpdir}/pkg.tar.gz" ]]; then
  echo "error: empty target download for ${TARGET_REL}" >&2
  exit 1
fi

# On-disk tree must match what is served for the root (transport integrity).
disk="sha256:$(shasum -a 256 "${REGISTRY_DIR}/metadata/root.json" | awk '{print $1}')"
if [[ "${disk}" != "${actual}" ]]; then
  echo "error: served root differs from ${REGISTRY_DIR}/metadata/root.json" >&2
  exit 1
fi

echo "ok base_url=${BASE_URL} root=${actual} target=${TARGET_REL}"
