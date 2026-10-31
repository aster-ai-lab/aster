#!/usr/bin/env bash
# First-principles gate for `just up::registry` and multi-package mock pubs.
#
# Mission: the just-started local transport (and ephemeral mock trees served
# the same way) are startable, byte-faithful, and consumable only under the
# pinned trust root — never because of the URL or just recipe name.
#
# Track J — just up::registry (committed Use-Registry tree)
#   J1 just up::registry becomes honestly ready
#   J2 smoke: served root == disk root == bootstrap pin
#   J3 consume selftest plan-install provenance == URL+root
#   J6 consume committed aster/applet-demo (ui/panel + targets↔archive digest)
#       then apply-plan → activity_bar entry digest == committed HTML
#   J4 just down::registry clears readiness
#   J5 wrong trust-root fails closed against the just-served URL
#
# Track S — supply drift / durable tip (no Use binary)
#   S0 git HEAD targets.json advertises aster/applet-demo archive (clean-clone P5)
#   S1 packages/applet-demo/aster-use-extension.acl == archive ACL bytes
#      (prevents bind_tool / surface drift after custody lag)
#
# Track M — mocked multi-package publication (alpha, beta, echo, compose)
#   M1 assemble+verify four mock packages
#   M2 serve mock tree via ASTER_USE_REGISTRY_DIR (same serve_local)
#   M3 smoke both skill archives + tool planning target
#   M4 plan-install alpha and beta with matching provenance
#   M5 unknown package fails plan
#   M6 wrong trust-root fails against mock URL
#   M7 stop clears mock readiness
#
# Track C — OKF + Skill + Tool + UI + MCP cooperation (aster/mock-compose)
#   C1 catalog surfaces include tool/mcp/okf/skill/ui with skill requires
#   C2 planning target covers tool + mcp executables
#   C3 plan-install compose provenance
#   C4 skill/ui require edges present in plan catalog
#   C5 broken requires_tool (missing id) fails assemble
#
# Track A — committed admissions include Applet UI supply (aster/applet-demo)
#   A1 assemble+verify admissions.acl includes applet-demo
#   A2 planning covers tool+mcp executables for applet-demo
#   A3 plan-install applet-demo; catalog has ui/panel with Tool/MCP/Skill binds
#   A4 apply-plan → capability snapshot activity_bar matches signed package bytes
#       (deps = UI bind_tool + bind_mcp + skill)
#
# Requires (from monorepo): just, python3, curl; ASTER_USE_BIN (0.3.x+);
# ASTER_USE_REGISTRY_TOOLS_BIN or a built aster-use-registry-tools.
set -euo pipefail

REGISTRY_REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MONOREPO="$(cd "${REGISTRY_REPO}/.." && pwd)"
SERVE="${REGISTRY_REPO}/scripts/serve_local.sh"
SMOKE="${REGISTRY_REPO}/scripts/smoke_local.sh"
ASSEMBLE_MOCK="${REGISTRY_REPO}/scripts/assemble_mock_registry.sh"
COMMITTED_ROOT="${ASTER_USE_REGISTRY_EXPECTED_ROOT:-sha256:ff399c6a599fd2acea8c3838c6efd073e87321413448c77dfa3b1616d076d172}"

PASS=0
FAIL=0
SKIP=0

WORK="$(mktemp -d "${TMPDIR:-/tmp}/aster-just-registry-gate.XXXXXX")"
JUST_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
MOCK_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')"
JUST_STATE="${WORK}/just-serve"
MOCK_STATE="${WORK}/mock-serve"
MOCK_ROOT="${WORK}/mock-pub"

cleanup() {
  (
    export ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}"
    export ASTER_USE_REGISTRY_PORT="${JUST_PORT}"
    "${SERVE}" stop >/dev/null 2>&1 || true
  )
  (
    export ASTER_USE_REGISTRY_STATE_DIR="${MOCK_STATE}"
    export ASTER_USE_REGISTRY_PORT="${MOCK_PORT}"
    "${SERVE}" stop >/dev/null 2>&1 || true
  )
  # Prefer just down when the recipe owns the process; ignore failures.
  (
    cd "${MONOREPO}"
    export ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}"
    export ASTER_USE_REGISTRY_PORT="${JUST_PORT}"
    just down::registry >/dev/null 2>&1 || true
  )
  rm -rf "${WORK}"
}
trap cleanup EXIT

pass() { PASS=$((PASS + 1)); printf 'PASS  %s\n' "$1"; }
fail() {
  FAIL=$((FAIL + 1))
  printf 'FAIL  %s\n' "$1" >&2
  [[ -n "${2:-}" ]] && printf '  %s\n' "$2" >&2
}
skip() {
  SKIP=$((SKIP + 1))
  printf 'SKIP  %s\n' "$1"
  [[ -n "${2:-}" ]] && printf '  %s\n' "$2"
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
    "${MONOREPO}/crates/use/target/debug/aster-use" \
    "${MONOREPO}/crates/use/target/release/aster-use"; do
    if [[ -x "${candidate}" ]] && "${candidate}" registry source list --json >/dev/null 2>&1; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}

plan_install() {
  local home="$1" url="$2" root="$3" package="$4" source_name="$5"
  export ASTER_USE_HOME="${home}"
  "${USE_BIN}" registry source add "${source_name}" \
    --url "${url}" \
    --trust-root "${root}" \
    --json >/dev/null
  "${USE_BIN}" plugin plan-install "${package}" \
    --registry-name "${source_name}" \
    --scope-kind user \
    --scope-id "user/registry-gate" \
    --json
}

check_plan_provenance() {
  local plan_json="$1" expected_url="$2" expected_root="$3" expected_pkg="$4"
  EXPECTED_URL="${expected_url}" EXPECTED_ROOT="${expected_root}" EXPECTED_PKG="${expected_pkg}" \
    PLAN_JSON="${plan_json}" python3 -c '
import json, os, sys
payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit(f"plan failed: {payload}")
data = payload["data"]
prov = data["plan"]["packageLock"]["packages"][0]["catalog"]["provenance"]
url = prov["registryUrl"].rstrip("/") + "/"
root = prov["rootSha256"]
if not root.startswith("sha256:"):
    root = "sha256:" + root
exp_url = os.environ["EXPECTED_URL"].rstrip("/") + "/"
exp_root = os.environ["EXPECTED_ROOT"]
if not exp_root.startswith("sha256:"):
    exp_root = "sha256:" + exp_root
if url != exp_url:
    raise SystemExit(f"url mismatch {url!r} != {exp_url!r}")
if root != exp_root:
    raise SystemExit(f"root mismatch {root!r} != {exp_root!r}")
if data["packageId"] != os.environ["EXPECTED_PKG"]:
    raise SystemExit(f"package mismatch {data['packageId']!r}")
'
}

printf 'just-registry first-principles gate\n'
printf 'monorepo=%s just_port=%s mock_port=%s\n' "${MONOREPO}" "${JUST_PORT}" "${MOCK_PORT}"

# --- Track J: just up::registry ------------------------------------------------
export ASTER_USE_REGISTRY_HOST=127.0.0.1
export ASTER_USE_REGISTRY_PORT="${JUST_PORT}"
export ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}"
unset ASTER_USE_REGISTRY_DIR || true
export ASTER_USE_REGISTRY_EXPECTED_ROOT="${COMMITTED_ROOT}"

assert_fails "J4 status fails before just up" env \
  ASTER_USE_REGISTRY_PORT="${JUST_PORT}" \
  ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}" \
  "${SERVE}" status

(
  cd "${MONOREPO}"
  export ASTER_USE_REGISTRY_HOST=127.0.0.1
  export ASTER_USE_REGISTRY_PORT="${JUST_PORT}"
  export ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}"
  just up::registry
) >/dev/null
assert_ok "J1 just up::registry status ready" env \
  ASTER_USE_REGISTRY_PORT="${JUST_PORT}" \
  ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}" \
  "${SERVE}" status

JUST_URL="$(
  ASTER_USE_REGISTRY_PORT="${JUST_PORT}" \
  ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}" \
  "${SERVE}" url
)"
case "${JUST_URL}" in
  http://127.0.0.1:"${JUST_PORT}"/) pass "J1 just url shape ${JUST_URL}" ;;
  *) fail "J1 just url shape" "got ${JUST_URL}" ;;
esac

assert_ok "J2 smoke committed tree via just transport" env \
  ASTER_USE_REGISTRY_EXPECTED_ROOT="${COMMITTED_ROOT}" \
  ASTER_USE_REGISTRY_DIR="${REGISTRY_REPO}/registry" \
  "${SMOKE}" "${JUST_URL}"

# S1 — first-principles: package source ACL must equal committed archive ACL.
# Custody republish is the only way to update the archive; source edits that
# leave the signed tree behind must fail this gate (no Host fixture, no soft skip).
set +e
SUPPLY_DRIFT="$(
  REGISTRY_DIR="${REGISTRY_REPO}/registry" \
  PACKAGE_ACL="${REGISTRY_REPO}/packages/applet-demo/aster-use-extension.acl" \
  python3 - <<'PY'
import hashlib, os, tarfile
from pathlib import Path

src = Path(os.environ["PACKAGE_ACL"])
if not src.is_file():
    raise SystemExit(f"missing package source ACL: {src}")
source = src.read_bytes()
root = Path(os.environ["REGISTRY_DIR"])
archive = next(root.glob("targets/extensions/aster/applet-demo/**/aster-applet-demo-*.tar.gz"))
with tarfile.open(archive, "r:gz") as tar:
    member = next(m for m in tar.getmembers() if m.name.endswith("aster-use-extension.acl"))
    archived = tar.extractfile(member).read()
if source != archived:
    raise SystemExit(
        "package source ACL drifted from committed archive ACL "
        f"(source_sha256={hashlib.sha256(source).hexdigest()} "
        f"archive_sha256={hashlib.sha256(archived).hexdigest()} "
        f"archive={archive}); republish with custody assemble"
    )
if b'bind_tool   = ["echo"]' not in archived:
    raise SystemExit('committed archive ACL missing bind_tool=["echo"]')
print("ok")
PY
)"
SUPPLY_DRIFT_EC=$?
set -e
if [[ "${SUPPLY_DRIFT_EC}" -eq 0 ]]; then
  pass "S1 applet-demo package source ACL equals committed archive"
else
  fail "S1 applet-demo package source ACL equals committed archive" "${SUPPLY_DRIFT}"
fi

# S0 — durable tip: git HEAD targets metadata must advertise applet-demo.
# Working-tree-only custody leaves clean clones without P5 supply; fail closed.
set +e
HEAD_SUPPLY="$(
  cd "${REGISTRY_REPO}" && python3 - <<'PY'
import json, subprocess, sys

proc = subprocess.run(
    ["git", "show", "HEAD:registry/metadata/targets.json"],
    capture_output=True,
    text=True,
)
if proc.returncode != 0:
    raise SystemExit(f"git show HEAD targets.json failed: {proc.stderr.strip()}")
targets = json.loads(proc.stdout)["signed"]["targets"]
keys = [k for k in targets if "applet-demo" in k and k.endswith(".tar.gz")]
if not keys:
    raise SystemExit(
        "HEAD registry/metadata/targets.json has no aster/applet-demo archive; "
        "commit packages/applet-demo + signed registry/targets + metadata "
        "(custody assemble) so clean checkouts keep P5 supply"
    )
print("ok")
PY
)"
HEAD_SUPPLY_EC=$?
set -e
if [[ "${HEAD_SUPPLY_EC}" -eq 0 ]]; then
  pass "S0 HEAD targets.json advertises committed applet-demo archive"
else
  fail "S0 HEAD targets.json advertises committed applet-demo archive" "${HEAD_SUPPLY}"
fi

USE_BIN=""
if USE_BIN="$(resolve_use_bin)"; then
  export ASTER_USE_BIN="${USE_BIN}"
  HOME_J="$(mktemp -d "${WORK}/home-just.XXXXXX")"
  set +e
  PLAN_J="$(plan_install "${HOME_J}" "${JUST_URL}" "${COMMITTED_ROOT}" "aster/registry-selftest" "just-local" 2>&1)"
  PLAN_J_EC=$?
  set -e
  if [[ "${PLAN_J_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_J}" "${JUST_URL}" "${COMMITTED_ROOT}" "aster/registry-selftest"; then
    pass "J3 consume selftest provenance"
  else
    fail "J3 consume selftest provenance" "ec=${PLAN_J_EC} out=${PLAN_J}"
  fi

  # J6 — first-principles: committed tree (not fresh admissions assemble) supplies applet-demo
  HOME_JA="$(mktemp -d "${WORK}/home-just-applet.XXXXXX")"
  set +e
  PLAN_JA="$(plan_install "${HOME_JA}" "${JUST_URL}" "${COMMITTED_ROOT}" "aster/applet-demo" "just-applet" 2>&1)"
  PLAN_JA_EC=$?
  set -e
  if [[ "${PLAN_JA_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_JA}" "${JUST_URL}" "${COMMITTED_ROOT}" "aster/applet-demo"; then
    pass "J6 plan-install committed applet-demo provenance"
  else
    fail "J6 plan-install committed applet-demo provenance" "ec=${PLAN_JA_EC} out=${PLAN_JA}"
  fi
  set +e
  APPLET_COMMITTED="$(
    PLAN_JSON="${PLAN_JA}" REGISTRY_DIR="${REGISTRY_REPO}/registry" python3 - <<'PY'
import hashlib, json, os, tarfile
from pathlib import Path

payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit("plan not ok")
catalog = payload["data"]["plan"]["packageLock"]["packages"][0]["catalog"]
surfaces = []
if isinstance(catalog, dict):
    surfaces = catalog.get("record", {}).get("surfaces") or catalog.get("surfaces") or []
ids = {(s.get("kind"), s.get("id")) for s in surfaces}
need = {
    ("tool", "echo"),
    ("mcp", "context"),
    ("skill", "applet-demo"),
    ("ui", "panel"),
}
missing = need - ids
if missing:
    raise SystemExit(f"missing surfaces {sorted(missing)}; have {sorted(ids)}")
ui = next(s for s in surfaces if s.get("kind") == "ui" and s.get("id") == "panel")
requires = {(r.get("kind"), r.get("id")) for r in ui.get("requires", [])}
# Committed archive must match package source: UI binds Executable Tool echo +
# MCP context + Skill applet-demo (same contract as Track A A3).
need_req = {("tool", "echo"), ("mcp", "context"), ("skill", "applet-demo")}
missing_req = need_req - requires
if missing_req:
    raise SystemExit(f"ui missing requires {sorted(missing_req)}; have {sorted(requires)}")

root = Path(os.environ["REGISTRY_DIR"])
archive = next(root.glob("targets/extensions/aster/applet-demo/**/aster-applet-demo-*.tar.gz"))
archive_bytes = archive.read_bytes()
archive_digest = hashlib.sha256(archive_bytes).hexdigest()
targets = json.loads((root / "metadata" / "targets.json").read_text())["signed"]["targets"]
target_key = next(k for k in targets if k.endswith("aster-applet-demo-0.1.0-any.tar.gz"))
meta_hash = targets[target_key]["hashes"]["sha256"]
if meta_hash != archive_digest:
    raise SystemExit(
        f"targets.json sha256 {meta_hash!r} != committed archive file {archive_digest!r}"
    )
with tarfile.open(archive, "r:gz") as tar:
    member = next(m for m in tar.getmembers() if m.name.endswith("ui/panel/index.html"))
    html = tar.extractfile(member).read()
    acl_member = next(m for m in tar.getmembers() if m.name.endswith("aster-use-extension.acl"))
    manifest = tar.extractfile(acl_member).read().decode()
if b"Applet Demo" not in html:
    raise SystemExit("committed archive HTML missing package title bytes")
if 'bind_tool   = ["echo"]' not in manifest:
    raise SystemExit("committed archive manifest missing bind_tool=[\"echo\"]")
print(json.dumps({"htmlSha256": hashlib.sha256(html).hexdigest(), "archiveSha256": archive_digest}))
PY
  )"
  APPLET_COMMITTED_EC=$?
  set -e
  if [[ "${APPLET_COMMITTED_EC}" -eq 0 ]]; then
    pass "J6 committed applet-demo catalog ui/panel + archive bytes"
  else
    fail "J6 committed applet-demo catalog ui/panel + archive bytes" "${APPLET_COMMITTED}"
  fi

  # J6b — apply from committed tree; activity_bar entry digest == committed HTML
  set +e
  APPLET_APPLY="$(
    PLAN_JSON="${PLAN_JA}" HOME_JSON="${HOME_JA}" USE_BIN_JSON="${USE_BIN}" \
    REGISTRY_DIR="${REGISTRY_REPO}/registry" python3 - <<'PY'
import hashlib, json, os, subprocess, tarfile
from pathlib import Path

payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit(f"plan not ok: {payload}")
plan = payload["data"]["plan"]
operation_id = plan["plan"]["operationId"]
plan_digest = plan["planDigest"]
env = os.environ.copy()
env["ASTER_USE_HOME"] = os.environ["HOME_JSON"]
proc = subprocess.run(
    [
        os.environ["USE_BIN_JSON"],
        "plugin",
        "apply-plan",
        "--operation-id",
        operation_id,
        "--plan-digest",
        plan_digest,
        "--scope-kind",
        "user",
        "--scope-id",
        "user/registry-gate",
        "--yes",
        "--json",
    ],
    env=env,
    capture_output=True,
    text=True,
)
if proc.returncode != 0:
    raise SystemExit(f"apply-plan failed ec={proc.returncode} out={proc.stdout} err={proc.stderr}")
applied = json.loads(proc.stdout)
if not applied.get("ok"):
    raise SystemExit(f"apply-plan not ok: {applied}")
state = applied["data"].get("state") or {}
if state.get("observed") != "ready" or state.get("desired") != "enabled":
    raise SystemExit(f"apply state not ready/enabled: {state}")

proc = subprocess.run(
    [
        os.environ["USE_BIN_JSON"],
        "capability",
        "snapshot",
        "--scope-kind",
        "user",
        "--scope-id",
        "user/registry-gate",
        "--json",
    ],
    env=env,
    capture_output=True,
    text=True,
)
if proc.returncode != 0:
    raise SystemExit(f"capability snapshot failed ec={proc.returncode} out={proc.stdout} err={proc.stderr}")
snap = json.loads(proc.stdout)
if not snap.get("ok"):
    raise SystemExit(f"snapshot not ok: {snap}")
caps = snap["data"]["registry"].get("capabilities") or []
applet = next((c for c in caps if c.get("alias") == "applet-demo"), None)
if applet is None:
    raise SystemExit(f"missing applet-demo capability; aliases={[c.get('alias') for c in caps]}")
if not applet.get("enabled"):
    raise SystemExit(f"applet-demo capability not enabled: {applet.get('readiness')}")
bars = applet.get("activityBar") or []
if len(bars) != 1:
    raise SystemExit(f"expected one activity_bar entry, got {len(bars)}")
bar = bars[0]
if bar.get("id") != "panel" or bar.get("title") != "Applet Demo" or bar.get("icon") != "layout" or bar.get("order") != 20:
    raise SystemExit(f"activity_bar presentation drifted: {bar}")
need_deps = {("tool", "echo"), ("mcp", "context"), ("skill", "applet-demo")}
have_deps = {(d.get("kind"), d.get("id")) for d in bar.get("dependencies") or []}
missing = need_deps - have_deps
if missing:
    raise SystemExit(f"activity_bar missing deps {sorted(missing)}; have {sorted(have_deps)}")
entry_digest = (bar.get("entry") or {}).get("sha256")
if not entry_digest:
    raise SystemExit("activity_bar entry digest missing")
root = Path(os.environ["REGISTRY_DIR"])
archive = next(root.glob("targets/extensions/aster/applet-demo/**/aster-applet-demo-*.tar.gz"))
with tarfile.open(archive, "r:gz") as tar:
    member = next(m for m in tar.getmembers() if m.name.endswith("ui/panel/index.html"))
    html = tar.extractfile(member).read()
expected = hashlib.sha256(html).hexdigest()
if entry_digest != expected:
    raise SystemExit(
        f"activity_bar entry digest {entry_digest!r} != committed archive HTML {expected!r}"
    )
print("ok")
PY
  )"
  APPLET_APPLY_EC=$?
  set -e
  if [[ "${APPLET_APPLY_EC}" -eq 0 ]]; then
    pass "J6 apply-plan committed applet-demo activity_bar matches archive"
  else
    fail "J6 apply-plan committed applet-demo activity_bar matches archive" "${APPLET_APPLY}"
  fi

  HOME_JBAD="$(mktemp -d "${WORK}/home-just-bad.XXXXXX")"
  set +e
  BAD_J="$(plan_install "${HOME_JBAD}" "${JUST_URL}" \
    "sha256:0000000000000000000000000000000000000000000000000000000000000000" \
    "aster/registry-selftest" "just-bad" 2>&1)"
  BAD_J_EC=$?
  set -e
  if [[ "${BAD_J_EC}" -ne 0 ]]; then
    pass "J5 wrong trust-root fails on just URL"
  else
    fail "J5 wrong trust-root fails on just URL" "${BAD_J}"
  fi
else
  skip "J3 consume selftest provenance" "set ASTER_USE_BIN"
  skip "J6 plan-install committed applet-demo provenance" "set ASTER_USE_BIN"
  skip "J6 committed applet-demo catalog ui/panel + archive bytes" "set ASTER_USE_BIN"
  skip "J6 apply-plan committed applet-demo activity_bar matches archive" "set ASTER_USE_BIN"
  skip "J5 wrong trust-root fails on just URL" "requires ASTER_USE_BIN"
fi

(
  cd "${MONOREPO}"
  export ASTER_USE_REGISTRY_PORT="${JUST_PORT}"
  export ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}"
  just down::registry
) >/dev/null
assert_fails "J4 just down::registry clears readiness" env \
  ASTER_USE_REGISTRY_PORT="${JUST_PORT}" \
  ASTER_USE_REGISTRY_STATE_DIR="${JUST_STATE}" \
  "${SERVE}" status

# --- Track M: mock multi-package tree ----------------------------------------
chmod +x "${ASSEMBLE_MOCK}"
MOCK_META="$("${ASSEMBLE_MOCK}" "${MOCK_ROOT}")"
MOCK_SHA="$(printf '%s\n' "${MOCK_META}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["rootSha256"])')"
MOCK_DIR="$(printf '%s\n' "${MOCK_META}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["registry"])')"
PKG_COUNT="$(printf '%s\n' "${MOCK_META}" | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))')"
if [[ "${PKG_COUNT}" -eq 4 && -f "${MOCK_DIR}/metadata/root.json" ]]; then
  pass "M1 assemble+verify four mock packages"
else
  fail "M1 assemble+verify four mock packages" "${MOCK_META}"
fi

export ASTER_USE_REGISTRY_DIR="${MOCK_DIR}"
export ASTER_USE_REGISTRY_PORT="${MOCK_PORT}"
export ASTER_USE_REGISTRY_STATE_DIR="${MOCK_STATE}"
export ASTER_USE_REGISTRY_EXPECTED_ROOT="${MOCK_SHA}"
export ASTER_USE_REGISTRY_SMOKE_TARGET="targets/extensions/aster/mock-alpha/0.1.0/stable/any/aster-mock-alpha-0.1.0-any.tar.gz"

assert_ok "M2 start mock registry transport" "${SERVE}" start
MOCK_URL="$("${SERVE}" url)"

assert_ok "M3 smoke mock-alpha target" "${SMOKE}" "${MOCK_URL}"
assert_ok "M3 download mock-beta archive" curl -fsS \
  "${MOCK_URL}targets/extensions/aster/mock-beta/0.1.0/stable/any/aster-mock-beta-0.1.0-any.tar.gz" \
  -o "${WORK}/mock-beta.tar.gz"
assert_ok "M3 download mock-echo planning target" curl -fsS \
  "${MOCK_URL}targets/extensions/aster/mock-echo/0.1.0/stable/any/planning-v1.json" \
  -o "${WORK}/mock-echo-planning.json"
assert_ok "C2 download mock-compose archive" curl -fsS \
  "${MOCK_URL}targets/extensions/aster/mock-compose/0.1.0/stable/any/aster-mock-compose-0.1.0-any.tar.gz" \
  -o "${WORK}/mock-compose.tar.gz"
assert_ok "C2 download mock-compose planning target" curl -fsS \
  "${MOCK_URL}targets/extensions/aster/mock-compose/0.1.0/stable/any/planning-v1.json" \
  -o "${WORK}/mock-compose-planning.json"

# Planning bundle must list both executable surfaces used by Skill/UI binds.
set +e
PLANNING_CHECK="$(
  COMPOSE_PLANNING="${WORK}/mock-compose-planning.json" python3 - <<'PY'
import json, os, sys
bundle = json.load(open(os.environ["COMPOSE_PLANNING"]))
kinds = {s.get("kind") for s in bundle.get("surfaces", [])}
ids = {s.get("id") for s in bundle.get("surfaces", [])}
need_kinds = {"tool-task-native", "mcp-stdio"}
need_ids = {"echo", "context"}
missing_kinds = need_kinds - kinds
missing_ids = need_ids - ids
if missing_kinds or missing_ids:
    print(f"kinds={sorted(kinds)} missing_kinds={sorted(missing_kinds)} ids={sorted(ids)} missing_ids={sorted(missing_ids)}")
    sys.exit(1)
print("ok")
PY
)"
PLANNING_EC=$?
set -e
if [[ "${PLANNING_EC}" -eq 0 ]]; then
  pass "C2 planning covers tool+mcp executables"
else
  fail "C2 planning covers tool+mcp executables" "${PLANNING_CHECK}"
fi

if [[ -n "${USE_BIN}" ]]; then
  HOME_MA="$(mktemp -d "${WORK}/home-alpha.XXXXXX")"
  set +e
  PLAN_A="$(plan_install "${HOME_MA}" "${MOCK_URL}" "${MOCK_SHA}" "aster/mock-alpha" "mock" 2>&1)"
  PLAN_A_EC=$?
  set -e
  if [[ "${PLAN_A_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_A}" "${MOCK_URL}" "${MOCK_SHA}" "aster/mock-alpha"; then
    pass "M4 plan-install mock-alpha provenance"
  else
    fail "M4 plan-install mock-alpha provenance" "ec=${PLAN_A_EC} out=${PLAN_A}"
  fi

  # Same home/source: second package must resolve without re-adding.
  set +e
  PLAN_B="$(
    ASTER_USE_HOME="${HOME_MA}" "${USE_BIN}" plugin plan-install aster/mock-beta \
      --registry-name mock \
      --scope-kind user \
      --scope-id user/registry-gate \
      --json 2>&1
  )"
  PLAN_B_EC=$?
  set -e
  if [[ "${PLAN_B_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_B}" "${MOCK_URL}" "${MOCK_SHA}" "aster/mock-beta"; then
    pass "M4 plan-install mock-beta provenance"
  else
    fail "M4 plan-install mock-beta provenance" "ec=${PLAN_B_EC} out=${PLAN_B}"
  fi

  set +e
  PLAN_C="$(
    ASTER_USE_HOME="${HOME_MA}" "${USE_BIN}" plugin plan-install aster/mock-compose \
      --registry-name mock \
      --scope-kind user \
      --scope-id user/registry-gate \
      --json 2>&1
  )"
  PLAN_C_EC=$?
  set -e
  if [[ "${PLAN_C_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_C}" "${MOCK_URL}" "${MOCK_SHA}" "aster/mock-compose"; then
    pass "C3 plan-install mock-compose provenance"
  else
    fail "C3 plan-install mock-compose provenance" "ec=${PLAN_C_EC} out=${PLAN_C}"
  fi

  set +e
  COMPOSE_SURFACES="$(
    PLAN_JSON="${PLAN_C}" python3 - <<'PY'
import json, os, sys
payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit("plan not ok")
catalog = payload["data"]["plan"]["packageLock"]["packages"][0]["catalog"]
surfaces = catalog.get("surfaces") or catalog.get("record", {}).get("surfaces") or []
# catalog may be nested under record depending on lock shape
if not surfaces and isinstance(catalog, dict):
    for key in ("surfaces",):
        if key in catalog:
            surfaces = catalog[key]
kinds = {s.get("kind") for s in surfaces}
ids = {(s.get("kind"), s.get("id")) for s in surfaces}
need = {
    ("tool", "echo"),
    ("mcp", "context"),
    ("okf", "domain"),
    ("skill", "compose"),
    ("ui", "compose"),
}
missing = need - ids
if missing:
    raise SystemExit(f"missing surfaces {sorted(missing)}; have {sorted(ids)}")
skill = next(s for s in surfaces if s.get("kind") == "skill" and s.get("id") == "compose")
requires = {(r.get("kind"), r.get("id")) for r in skill.get("requires", [])}
need_req = {("tool", "echo"), ("mcp", "context"), ("okf", "domain")}
missing_req = need_req - requires
if missing_req:
    raise SystemExit(f"skill missing requires {sorted(missing_req)}; have {sorted(requires)}")
print("ok")
PY
  )"
  COMPOSE_SURFACES_EC=$?
  set -e
  if [[ "${COMPOSE_SURFACES_EC}" -eq 0 ]]; then
    pass "C1/C4 compose surfaces + skill requires"
  else
    fail "C1/C4 compose surfaces + skill requires" "${COMPOSE_SURFACES} plan=${PLAN_C}"
  fi

  set +e
  PLAN_MISS="$(
    ASTER_USE_HOME="${HOME_MA}" "${USE_BIN}" plugin plan-install aster/does-not-exist \
      --registry-name mock \
      --scope-kind user \
      --scope-id user/registry-gate \
      --json 2>&1
  )"
  PLAN_MISS_EC=$?
  set -e
  if [[ "${PLAN_MISS_EC}" -ne 0 ]]; then
    pass "M5 unknown package fails plan"
  else
    fail "M5 unknown package fails plan" "${PLAN_MISS}"
  fi

  HOME_MBAD="$(mktemp -d "${WORK}/home-mock-bad.XXXXXX")"
  set +e
  BAD_M="$(plan_install "${HOME_MBAD}" "${MOCK_URL}" \
    "sha256:0000000000000000000000000000000000000000000000000000000000000000" \
    "aster/mock-alpha" "mock-bad" 2>&1)"
  BAD_M_EC=$?
  set -e
  if [[ "${BAD_M_EC}" -ne 0 ]]; then
    pass "M6 wrong trust-root fails on mock URL"
  else
    fail "M6 wrong trust-root fails on mock URL" "${BAD_M}"
  fi
else
  skip "M4 plan-install mock-alpha provenance" "requires ASTER_USE_BIN"
  skip "M4 plan-install mock-beta provenance" "requires ASTER_USE_BIN"
  skip "C3 plan-install mock-compose provenance" "requires ASTER_USE_BIN"
  skip "C1/C4 compose surfaces + skill requires" "requires ASTER_USE_BIN"
  skip "M5 unknown package fails plan" "requires ASTER_USE_BIN"
  skip "M6 wrong trust-root fails on mock URL" "requires ASTER_USE_BIN"
fi

# C5: Skill requires a tool id that does not exist in the package → assemble fails.
BROKEN="${WORK}/broken-compose"
mkdir -p "${BROKEN}/packages/broken/skills/x"
cp -R "${MOCK_ROOT}/packages/mock-compose/." "${BROKEN}/packages/broken/" 2>/dev/null || true
# Rebuild a minimal broken skill-only-requires-missing-tool package
rm -rf "${BROKEN}"
mkdir -p "${BROKEN}/packages/broken/skills/x"
cat >"${BROKEN}/packages/broken/aster-use-extension.acl" <<'EOF'
extension "aster/mock-broken" {
  schema_version = 3
  version        = "0.1.0"
  route          = "mock-broken"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/aster-ai-lab/Use-Registry"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  skill "broken" {
    path          = "skills/x/SKILL.md"
    requires_tool = ["does-not-exist"]
    requires_mcp  = []
    requires_okf  = []
    optional      = false
  }
}
EOF
echo '# broken' >"${BROKEN}/packages/broken/skills/x/SKILL.md"
echo '# broken' >"${BROKEN}/packages/broken/README.md"
cat >"${BROKEN}/admissions.acl" <<'EOF'
admission "aster/mock-broken" {
  package_directory = "packages/broken"
  channel           = "stable"
  target            = "any"
  display_name      = "Mock Broken"
  description       = "Skill requires a missing tool id."
  license           = "Apache-2.0"
  keywords          = ["mock"]
  categories        = ["testing"]
}
EOF
TOOLS_BIN="${ASTER_USE_REGISTRY_TOOLS_BIN:-}"
if [[ -z "${TOOLS_BIN}" || ! -x "${TOOLS_BIN}" ]]; then
  for candidate in \
    "${MONOREPO}/crates/use/target/release/aster-use-registry-tools" \
    "${MONOREPO}/crates/use/target/debug/aster-use-registry-tools"; do
    if [[ -x "${candidate}" ]]; then
      TOOLS_BIN="${candidate}"
      break
    fi
  done
fi
if [[ -n "${TOOLS_BIN}" ]]; then
  "${TOOLS_BIN}" keygen --keys-dir "${BROKEN}/keys" >/dev/null
  set +e
  BROKEN_OUT="$("${TOOLS_BIN}" assemble \
    --keys-dir "${BROKEN}/keys" \
    --admissions "${BROKEN}/admissions.acl" \
    --out-root "${BROKEN}/registry" 2>&1)"
  BROKEN_EC=$?
  set -e
  if [[ "${BROKEN_EC}" -ne 0 ]]; then
    pass "C5 broken skill requires_tool fails assemble"
  else
    fail "C5 broken skill requires_tool fails assemble" "${BROKEN_OUT}"
  fi
else
  skip "C5 broken skill requires_tool fails assemble" "registry-tools missing"
fi

# --- Track A: committed admissions Applet UI supply --------------------------
# Mock transport still holds MOCK_PORT; release it before admissions serve.
"${SERVE}" stop >/dev/null 2>&1 || true
ADMISSIONS_ROOT="${WORK}/admissions-applet"
mkdir -p "${ADMISSIONS_ROOT}"
TOOLS_BIN="${ASTER_USE_REGISTRY_TOOLS_BIN:-}"
if [[ -z "${TOOLS_BIN}" || ! -x "${TOOLS_BIN}" ]]; then
  for candidate in \
    "${MONOREPO}/crates/use/target/release/aster-use-registry-tools" \
    "${MONOREPO}/crates/use/target/debug/aster-use-registry-tools"; do
    if [[ -x "${candidate}" ]]; then
      TOOLS_BIN="${candidate}"
      break
    fi
  done
fi
if [[ -n "${TOOLS_BIN}" ]]; then
  "${TOOLS_BIN}" keygen --keys-dir "${ADMISSIONS_ROOT}/keys" >/dev/null
  set +e
  ADMISSION_META="$("${TOOLS_BIN}" assemble \
    --keys-dir "${ADMISSIONS_ROOT}/keys" \
    --admissions "${REGISTRY_REPO}/admissions.acl" \
    --out-root "${ADMISSIONS_ROOT}/registry" 2>&1)"
  ADMISSION_EC=$?
  set -e
  if [[ "${ADMISSION_EC}" -eq 0 ]]; then
    "${TOOLS_BIN}" verify --registry "${ADMISSIONS_ROOT}/registry" >/dev/null
    pass "A1 assemble+verify admissions includes applet-demo"
  else
    fail "A1 assemble+verify admissions includes applet-demo" "${ADMISSION_META}"
  fi

  set +e
  APPLET_PLANNING_CHECK="$(
    REGISTRY_DIR="${ADMISSIONS_ROOT}/registry" python3 - <<'PY'
import json, os, pathlib, sys
root = pathlib.Path(os.environ["REGISTRY_DIR"])
planning = list(root.glob("targets/extensions/aster/applet-demo/**/planning-v1.json"))
if not planning:
    raise SystemExit("missing applet-demo planning-v1.json")
bundle = json.loads(planning[0].read_text())
kinds = {s.get("kind") for s in bundle.get("surfaces", [])}
ids = {s.get("id") for s in bundle.get("surfaces", [])}
need_kinds = {"tool-task-native", "mcp-stdio"}
need_ids = {"echo", "context"}
missing_kinds = need_kinds - kinds
missing_ids = need_ids - ids
if missing_kinds or missing_ids:
    raise SystemExit(
        f"kinds={sorted(kinds)} missing_kinds={sorted(missing_kinds)} "
        f"ids={sorted(ids)} missing_ids={sorted(missing_ids)}"
    )
print("ok")
PY
  )"
  APPLET_PLANNING_EC=$?
  set -e
  if [[ "${APPLET_PLANNING_EC}" -eq 0 ]]; then
    pass "A2 applet-demo planning covers tool+mcp"
  else
    fail "A2 applet-demo planning covers tool+mcp" "${APPLET_PLANNING_CHECK}"
  fi

  if [[ -n "${USE_BIN}" ]]; then
    ADMISSION_SHA="$(
      REGISTRY_DIR="${ADMISSIONS_ROOT}/registry" python3 - <<'PY'
import hashlib, pathlib, os
root = pathlib.Path(os.environ["REGISTRY_DIR"]) / "metadata" / "root.json"
digest = hashlib.sha256(root.read_bytes()).hexdigest()
print(f"sha256:{digest}")
PY
    )"
    export ASTER_USE_REGISTRY_DIR="${ADMISSIONS_ROOT}/registry"
    export ASTER_USE_REGISTRY_PORT="${MOCK_PORT}"
    export ASTER_USE_REGISTRY_STATE_DIR="${ADMISSIONS_ROOT}/state"
    export ASTER_USE_REGISTRY_EXPECTED_ROOT="${ADMISSION_SHA}"
    mkdir -p "${ADMISSIONS_ROOT}/state"
    assert_ok "A3 start admissions applet registry" "${SERVE}" start
    ADMISSION_URL="$("${SERVE}" url)"
    HOME_A="$(mktemp -d "${WORK}/home-applet.XXXXXX")"
    set +e
    PLAN_A="$(plan_install "${HOME_A}" "${ADMISSION_URL}" "${ADMISSION_SHA}" "aster/applet-demo" "applet-demo" 2>&1)"
    PLAN_A_EC=$?
    set -e
    if [[ "${PLAN_A_EC}" -eq 0 ]] && check_plan_provenance "${PLAN_A}" "${ADMISSION_URL}" "${ADMISSION_SHA}" "aster/applet-demo"; then
      pass "A3 plan-install applet-demo provenance"
    else
      fail "A3 plan-install applet-demo provenance" "ec=${PLAN_A_EC} out=${PLAN_A}"
    fi
    set +e
    APPLET_SURFACES="$(
      PLAN_JSON="${PLAN_A}" REGISTRY_DIR="${ADMISSIONS_ROOT}/registry" python3 - <<'PY'
import json, os, tarfile, io, pathlib, sys
payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit("plan not ok")
catalog = payload["data"]["plan"]["packageLock"]["packages"][0]["catalog"]
surfaces = []
if isinstance(catalog, dict):
    surfaces = catalog.get("record", {}).get("surfaces") or catalog.get("surfaces") or []
ids = {(s.get("kind"), s.get("id")) for s in surfaces}
need = {
    ("tool", "echo"),
    ("mcp", "context"),
    ("skill", "applet-demo"),
    ("ui", "panel"),
}
missing = need - ids
if missing:
    raise SystemExit(f"missing surfaces {sorted(missing)}; have {sorted(ids)}")
ui = next(s for s in surfaces if s.get("kind") == "ui" and s.get("id") == "panel")
requires = {(r.get("kind"), r.get("id")) for r in ui.get("requires", [])}
need_req = {("mcp", "context"), ("skill", "applet-demo"), ("tool", "echo")}
missing_req = need_req - requires
if missing_req:
    raise SystemExit(f"ui missing requires {sorted(missing_req)}; have {sorted(requires)}")
# UI binds package-local Executable Tool + MCP + Skill. Presentation metadata
# lives in the signed package manifest (activity_bar supply).
root = pathlib.Path(os.environ["REGISTRY_DIR"])
archive = next(root.glob("targets/extensions/aster/applet-demo/**/aster-applet-demo-*.tar.gz"))
with tarfile.open(archive, "r:gz") as tar:
    member = next(m for m in tar.getmembers() if m.name.endswith("aster-use-extension.acl"))
    manifest = tar.extractfile(member).read().decode()
for needle in ('title       = "Applet Demo"', 'icon        = "layout"', "order       = 20", 'bind_tool   = ["echo"]'):
    if needle not in manifest:
        raise SystemExit(f"package manifest missing {needle!r}")
print("ok")
PY
    )"
    APPLET_SURFACES_EC=$?
    set -e
    if [[ "${APPLET_SURFACES_EC}" -eq 0 ]]; then
      pass "A3 applet-demo catalog ui/panel + binds"
    else
      fail "A3 applet-demo catalog ui/panel + binds" "${APPLET_SURFACES} plan=${PLAN_A}"
    fi

    # A4 — real Plugin Manager apply; activity_bar entry digest == signed archive HTML
    set +e
    APPLY_META="$(
      PLAN_JSON="${PLAN_A}" HOME_JSON="${HOME_A}" USE_BIN_JSON="${USE_BIN}" python3 - <<'PY'
import json, os, subprocess, sys
payload = json.loads(os.environ["PLAN_JSON"])
if not payload.get("ok"):
    raise SystemExit(f"plan not ok: {payload}")
plan = payload["data"]["plan"]
operation_id = plan["plan"]["operationId"]
plan_digest = plan["planDigest"]
env = os.environ.copy()
env["ASTER_USE_HOME"] = os.environ["HOME_JSON"]
proc = subprocess.run(
    [
        os.environ["USE_BIN_JSON"],
        "plugin",
        "apply-plan",
        "--operation-id",
        operation_id,
        "--plan-digest",
        plan_digest,
        "--scope-kind",
        "user",
        "--scope-id",
        "user/registry-gate",
        "--yes",
        "--json",
    ],
    env=env,
    capture_output=True,
    text=True,
)
if proc.returncode != 0:
    raise SystemExit(f"apply-plan failed ec={proc.returncode} out={proc.stdout} err={proc.stderr}")
applied = json.loads(proc.stdout)
if not applied.get("ok"):
    raise SystemExit(f"apply-plan not ok: {applied}")
state = applied["data"].get("state") or {}
if state.get("observed") != "ready" or state.get("desired") != "enabled":
    raise SystemExit(f"apply state not ready/enabled: {state}")
print(json.dumps({"operationId": operation_id, "planDigest": plan_digest}))
PY
    )"
    APPLY_EC=$?
    set -e
    if [[ "${APPLY_EC}" -eq 0 ]]; then
      pass "A4 apply-plan applet-demo ready"
    else
      fail "A4 apply-plan applet-demo ready" "${APPLY_META}"
    fi

    set +e
    APPLET_ACTIVITY="$(
      HOME_JSON="${HOME_A}" USE_BIN_JSON="${USE_BIN}" REGISTRY_DIR="${ADMISSIONS_ROOT}/registry" python3 - <<'PY'
import hashlib, json, os, subprocess, tarfile
from pathlib import Path

env = os.environ.copy()
env["ASTER_USE_HOME"] = os.environ["HOME_JSON"]
proc = subprocess.run(
    [
        os.environ["USE_BIN_JSON"],
        "capability",
        "snapshot",
        "--scope-kind",
        "user",
        "--scope-id",
        "user/registry-gate",
        "--json",
    ],
    env=env,
    capture_output=True,
    text=True,
)
if proc.returncode != 0:
    raise SystemExit(f"capability snapshot failed ec={proc.returncode} out={proc.stdout} err={proc.stderr}")
snap = json.loads(proc.stdout)
if not snap.get("ok"):
    raise SystemExit(f"snapshot not ok: {snap}")
caps = snap["data"]["registry"].get("capabilities") or []
applet = next((c for c in caps if c.get("alias") == "applet-demo"), None)
if applet is None:
    raise SystemExit(f"missing applet-demo capability; aliases={[c.get('alias') for c in caps]}")
if not applet.get("enabled"):
    raise SystemExit(f"applet-demo capability not enabled: {applet.get('readiness')}")
bars = applet.get("activityBar") or []
if len(bars) != 1:
    raise SystemExit(f"expected one activity_bar entry, got {len(bars)}")
bar = bars[0]
if bar.get("id") != "panel" or bar.get("title") != "Applet Demo" or bar.get("icon") != "layout" or bar.get("order") != 20:
    raise SystemExit(f"activity_bar presentation drifted: {bar}")
# Match signed UI binds: Tool + MCP + Skill (canonical PluginSurfaceRef order).
need_deps = {("mcp", "context"), ("skill", "applet-demo"), ("tool", "echo")}
have_deps = {(d.get("kind"), d.get("id")) for d in bar.get("dependencies") or []}
missing = need_deps - have_deps
if missing:
    raise SystemExit(f"activity_bar missing deps {sorted(missing)}; have {sorted(have_deps)}")
entry_digest = (bar.get("entry") or {}).get("sha256")
if not entry_digest:
    raise SystemExit("activity_bar entry digest missing")
root = Path(os.environ["REGISTRY_DIR"])
archive = next(root.glob("targets/extensions/aster/applet-demo/**/aster-applet-demo-*.tar.gz"))
with tarfile.open(archive, "r:gz") as tar:
    member = next(m for m in tar.getmembers() if m.name.endswith("ui/panel/index.html"))
    html = tar.extractfile(member).read()
expected = hashlib.sha256(html).hexdigest()
if entry_digest != expected:
    raise SystemExit(
        f"activity_bar entry digest {entry_digest!r} != signed archive HTML {expected!r}"
    )
if b"Applet Demo" not in html:
    raise SystemExit("signed archive HTML missing package title bytes")
print("ok")
PY
    )"
    APPLET_ACTIVITY_EC=$?
    set -e
    if [[ "${APPLET_ACTIVITY_EC}" -eq 0 ]]; then
      pass "A4 applet-demo activity_bar matches signed UI bytes"
    else
      fail "A4 applet-demo activity_bar matches signed UI bytes" "${APPLET_ACTIVITY}"
    fi

    assert_ok "A3 stop admissions applet registry" "${SERVE}" stop
  else
    skip "A3 plan-install applet-demo provenance" "requires ASTER_USE_BIN"
    skip "A3 applet-demo catalog ui/panel + binds" "requires ASTER_USE_BIN"
    skip "A4 apply-plan applet-demo ready" "requires ASTER_USE_BIN"
    skip "A4 applet-demo activity_bar matches signed UI bytes" "requires ASTER_USE_BIN"
  fi
else
  skip "A1 assemble+verify admissions includes applet-demo" "registry-tools missing"
  skip "A2 applet-demo planning covers tool+mcp" "registry-tools missing"
  skip "A3 plan-install applet-demo provenance" "registry-tools missing"
  skip "A3 applet-demo catalog ui/panel + binds" "registry-tools missing"
  skip "A4 apply-plan applet-demo ready" "registry-tools missing"
  skip "A4 applet-demo activity_bar matches signed UI bytes" "registry-tools missing"
fi

assert_ok "M7 stop mock transport" "${SERVE}" stop
assert_fails "M7 status fails after mock stop" "${SERVE}" status

printf '\nsummary pass=%s fail=%s skip=%s\n' "${PASS}" "${FAIL}" "${SKIP}"
if [[ "${FAIL}" -ne 0 ]]; then
  exit 1
fi
