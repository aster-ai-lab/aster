#!/usr/bin/env bash
# Probe local spine health after `just up`. Exit 1 if any required probe fails.
# Wave A: also reports /live and etcd discovery keys under /a3s/services/.
set -euo pipefail

fail=0
ok() { printf '  OK   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; fail=1; }
soft() { printf '  WARN %s\n' "$1"; }

probe_http() {
  local url="$1"
  curl -sf --max-time 2 "$url" >/dev/null 2>&1
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
etcd_container="$(bash "$repo_root/scripts/resolve-container.sh" a3s-etcd a3s-etcd)"

# Returns 0 if etcd has at least one key under /a3s/services/<name>/
etcd_has_service() {
  local name="$1"
  local out
  out="$(docker exec "$etcd_container" etcdctl get --prefix --keys-only "/a3s/services/${name}/" 2>/dev/null || true)"
  [[ -n "${out//[[:space:]]/}" ]]
}

echo "spine status:"

if probe_http "http://127.0.0.1:2379/health" \
  || docker inspect -f '{{.State.Health.Status}}' "$etcd_container" 2>/dev/null | grep -qx healthy; then
  ok "a3s-etcd          :2379"
else
  bad "a3s-etcd          :2379"
fi

if probe_http "http://127.0.0.1:3000/live" && probe_http "http://127.0.0.1:3000/health"; then
  ok "a3s-api           :3000 live+health"
elif probe_http "http://127.0.0.1:3000/api/health" || probe_http "http://127.0.0.1:3000/health"; then
  soft "a3s-api           :3000 health only (rebuild for /live)"
else
  bad "a3s-api           :3000"
fi

if probe_http "http://127.0.0.1:8090/health"; then
  ok "a3s-search        :8090"
else
  bad "a3s-search        :8090"
fi

if probe_http "http://127.0.0.1:18080/api/v1/health/live" \
  && probe_http "http://127.0.0.1:18080/api/v1/health/ready"; then
  ok "a3s-cloud         :18080 live+ready"
else
  soft "a3s-cloud         :18080 (optional for Work /v1 e2e)"
fi

if probe_http "http://127.0.0.1:7641/live" && probe_http "http://127.0.0.1:7641/health"; then
  ok "a3s-auth          :7641 live+health"
else
  bad "a3s-auth          :7641"
fi

if probe_http "http://127.0.0.1:7651/live" && probe_http "http://127.0.0.1:7651/health"; then
  ok "a3s-billing       :7651 live+health"
else
  bad "a3s-billing       :7651"
fi

if probe_http "http://127.0.0.1:7652/live" && probe_http "http://127.0.0.1:7652/health"; then
  ok "a3s-config        :7652 live+health"
elif probe_http "http://127.0.0.1:7652/api/health" || probe_http "http://127.0.0.1:7652/health"; then
  soft "a3s-config        :7652 health only (rebuild for /live)"
else
  bad "a3s-config        :7652"
fi

tenant_note=""
stamp_file="$repo_root/apps/web/.data/tenant.stamp"
if [[ -f "$stamp_file" ]]; then
  tenant_note=" tenant=$(head -n 1 "$stamp_file")"
fi

if curl -sf --max-time 2 -o /dev/null "http://127.0.0.1:3100/"; then
  ok "a3s console     :3100${tenant_note}"
else
  bad "a3s console     :3100${tenant_note}"
fi

echo ""
echo "discovery (etcd /a3s/services):"
for pair in a3s-api:a3s-api a3s-auth:a3s-auth a3s-billing:a3s-billing a3s-config:a3s-config; do
  current="${pair%%:*}"
  previous="${pair##*:}"
  if etcd_has_service "$current"; then
    ok "registered ${current}"
  elif etcd_has_service "$previous"; then
    ok "registered ${previous}"
  else
    soft "missing  ${current} (set A3S_ETCD_ENDPOINTS + rebuild bins)"
  fi
done

echo ""
if [[ "$fail" -ne 0 ]]; then
  echo "required probes failed — see apps/api/.data/api.log · apps/config/.data/config.log · crates/search/.data/search.log" >&2
  exit 1
fi
echo "ready:"
echo "  a3s-etcd       → http://127.0.0.1:2379  (Boot discovery)"
echo "  a3s-api        → http://127.0.0.1:3000"
echo "  a3s-search     → http://127.0.0.1:8090"
echo "  a3s-cloud      → http://127.0.0.1:18080 (control plane, optional)"
echo "  a3s-auth       → http://127.0.0.1:7641"
echo "  a3s-billing    → http://127.0.0.1:7651"
echo "  a3s-config     → http://127.0.0.1:7652"
echo "  Console        → http://127.0.0.1:3100${tenant_note}"
