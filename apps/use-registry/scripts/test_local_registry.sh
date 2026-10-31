#!/usr/bin/env bash
# First-principles local registry gate.
#
# Mission: a local transport for the committed signed tree that is startable,
# honestly ready, byte-faithful, and consumable only under the pinned root.
#
# Invariants → cases:
#   I1 Transport is not trust          → wrong trust-root fails closed
#   I2 Served bytes == committed tree  → smoke root/target match disk
#   I3 Ready means our listener        → start/status only after our PID owns PORT
#   I4 Lifecycle is deterministic      → start idempotent; stop clears readiness
#   I5 Client consume uses TUF pin     → plan-install provenance matches URL+root
#   I6 Failures are loud               → smoke fails when stopped; busy port fails
#
# Optional: set ASTER_USE_BIN to a package-manager aster-use (0.3.x). Cases that
# need the client are skipped with an explicit note when unavailable.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVE="${ROOT_DIR}/scripts/serve_local.sh"
SMOKE="${ROOT_DIR}/scripts/smoke_local.sh"
CONSUME="${ROOT_DIR}/scripts/consume_local.sh"
EXPECTED_ROOT="${ASTER_USE_REGISTRY_EXPECTED_ROOT:-sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172}"

PASS=0
FAIL=0
SKIP=0

STATE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/aster-use-registry-test.XXXXXX")"
# Prefer an ephemeral high port to avoid colliding with a developer serve.
PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
export ASTER_USE_REGISTRY_HOST=127.0.0.1
export ASTER_USE_REGISTRY_PORT="${PORT}"
export ASTER_USE_REGISTRY_STATE_DIR="${STATE_DIR}/serve"
export ASTER_USE_REGISTRY_EXPECTED_ROOT="${EXPECTED_ROOT}"

cleanup() {
  "${SERVE}" stop >/dev/null 2>&1 || true
  rm -rf "${STATE_DIR}"
}
trap cleanup EXIT

note() { printf '  %s\n' "$*"; }

pass() {
  PASS=$((PASS + 1))
  printf 'PASS  %s\n' "$1"
}

fail() {
  FAIL=$((FAIL + 1))
  printf 'FAIL  %s\n' "$1" >&2
  if [[ -n "${2:-}" ]]; then
    note "$2"
  fi
}

skip() {
  SKIP=$((SKIP + 1))
  printf 'SKIP  %s\n' "$1"
  if [[ -n "${2:-}" ]]; then
    note "$2"
  fi
}

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [[ "${expected}" == "${actual}" ]]; then
    pass "${label}"
  else
    fail "${label}" "expected='${expected}' actual='${actual}'"
  fi
}

assert_ok() {
  local label="$1"
  shift
  local out ec=0
  set +e
  out="$("$@" 2>&1)"
  ec=$?
  set -e
  if [[ "${ec}" -eq 0 ]]; then
    pass "${label}"
  else
    fail "${label}" "exit=${ec} output=${out}"
  fi
}

assert_fails() {
  local label="$1"
  shift
  local out ec=0
  set +e
  out="$("$@" 2>&1)"
  ec=$?
  set -e
  if [[ "${ec}" -ne 0 ]]; then
    pass "${label}"
  else
    fail "${label}" "expected failure, got: ${out}"
  fi
}

resolve_use_bin() {
  if [[ -n "${ASTER_USE_BIN:-}" && -x "${ASTER_USE_BIN}" ]]; then
    if "${ASTER_USE_BIN}" registry source list --json >/dev/null 2>&1; then
      printf '%s\n' "${ASTER_USE_BIN}"
      return 0
    fi
  fi
  local candidate
  for candidate in \
    "${ROOT_DIR}/../crates/use/target/debug/aster-use" \
    "${ROOT_DIR}/../crates/use/target/release/aster-use"; do
    if [[ -x "${candidate}" ]] && "${candidate}" registry source list --json >/dev/null 2>&1; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}

printf 'local registry first-principles gate\n'
printf 'port=%s state=%s\n' "${PORT}" "${STATE_DIR}"

# --- I4 / I3 lifecycle -------------------------------------------------------
assert_fails "I4 status fails before start" "${SERVE}" status

assert_ok "I3 start becomes ready" "${SERVE}" start
assert_ok "I3 status ready after start" "${SERVE}" status

URL="$("${SERVE}" url)"
case "${URL}" in
  http://127.0.0.1:"${PORT}"/) pass "I3 url shape ${URL}" ;;
  *) fail "I3 url shape" "got ${URL}" ;;
esac

START_AGAIN="$("${SERVE}" start 2>&1 || true)"
case "${START_AGAIN}" in
  already\ running*) pass "I4 start is idempotent while healthy" ;;
  *) fail "I4 start is idempotent while healthy" "got ${START_AGAIN}" ;;
esac

# --- I2 transport integrity --------------------------------------------------
assert_ok "I2 smoke matches committed root+target" "${SMOKE}" "${URL}"

DISK_ROOT="sha256:$(shasum -a 256 "${ROOT_DIR}/registry/metadata/root.json" | awk '{print $1}')"
assert_eq "I2 bootstrap pin matches disk" "${EXPECTED_ROOT}" "${DISK_ROOT}"

# --- I6 loud failures --------------------------------------------------------
assert_ok "I4 stop succeeds" "${SERVE}" stop
assert_fails "I4 status fails after stop" "${SERVE}" status
assert_fails "I6 smoke fails when stopped" "${SMOKE}" "${URL}"

# Busy port: hold the port with a decoy, start must refuse.
python3 -m http.server "${PORT}" --bind 127.0.0.1 >/dev/null 2>&1 &
DECOY_PID=$!
sleep 0.3
assert_fails "I6 start refuses foreign busy port" "${SERVE}" start
kill "${DECOY_PID}" 2>/dev/null || true
wait "${DECOY_PID}" 2>/dev/null || true

# Stale PID pointing at unrelated alive process must not block a real start.
assert_ok "I3 start after busy-port clear" "${SERVE}" start
REAL_PID="$(tr -d '[:space:]' <"${ASTER_USE_REGISTRY_STATE_DIR}/httpd.pid")"
"${SERVE}" stop >/dev/null
# Fake a stale PID file with a long-lived process (this shell's parent is wrong;
# use `sleep` as a live unrelated PID).
sleep 120 &
STALE_PID=$!
mkdir -p "${ASTER_USE_REGISTRY_STATE_DIR}"
echo "${STALE_PID}" >"${ASTER_USE_REGISTRY_STATE_DIR}/httpd.pid"
assert_ok "I3 start recovers from stale unrelated PID" "${SERVE}" start
NEW_PID="$(tr -d '[:space:]' <"${ASTER_USE_REGISTRY_STATE_DIR}/httpd.pid")"
if [[ "${NEW_PID}" != "${STALE_PID}" ]] && kill -0 "${STALE_PID}" 2>/dev/null; then
  pass "I3 stale PID was not stolen"
else
  fail "I3 stale PID was not stolen" "new=${NEW_PID} stale=${STALE_PID}"
fi
kill "${STALE_PID}" 2>/dev/null || true
wait "${STALE_PID}" 2>/dev/null || true

# --- I1 / I5 client consume --------------------------------------------------
USE_BIN=""
if USE_BIN="$(resolve_use_bin)"; then
  export ASTER_USE_BIN="${USE_BIN}"
  assert_ok "I5 consume plan-install against local URL" "${CONSUME}"

  # Wrong trust root must fail closed at plan time (source may add; refresh fails).
  HOME_BAD="$(mktemp -d "${STATE_DIR}/bad-home.XXXXXX")"
  set +e
  BAD_OUT="$(
    ASTER_USE_HOME="${HOME_BAD}" "${USE_BIN}" registry source add local \
      --url "${URL}" \
      --trust-root "sha256:0000000000000000000000000000000000000000000000000000000000000000" \
      --json 2>&1
  )"
  BAD_ADD_EC=$?
  BAD_PLAN="$(
    ASTER_USE_HOME="${HOME_BAD}" "${USE_BIN}" plugin plan-install aster/registry-selftest \
      --registry-name local \
      --scope-kind user \
      --scope-id user/registry-local \
      --json 2>&1
  )"
  BAD_PLAN_EC=$?
  set -e
  if [[ "${BAD_ADD_EC}" -ne 0 || "${BAD_PLAN_EC}" -ne 0 ]]; then
    pass "I1 wrong trust-root fails closed (add_ec=${BAD_ADD_EC} plan_ec=${BAD_PLAN_EC})"
  else
    fail "I1 wrong trust-root fails closed" "add=${BAD_OUT} plan=${BAD_PLAN}"
  fi
else
  skip "I5 consume plan-install against local URL" "set ASTER_USE_BIN to Use package-manager 0.3.x"
  skip "I1 wrong trust-root fails closed" "requires ASTER_USE_BIN"
fi

assert_ok "I4 final stop" "${SERVE}" stop

printf '\nsummary pass=%s fail=%s skip=%s\n' "${PASS}" "${FAIL}" "${SKIP}"
if [[ "${FAIL}" -ne 0 ]]; then
  exit 1
fi
