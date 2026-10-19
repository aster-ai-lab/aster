#!/usr/bin/env bash
# Create per-service databases on the shared Postgres from the OS docker-compose.yml.
# One server. Separate databases and roles for api, cloud, auth, billing, and config.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

for helper in compose-postgres-host.sh resolve-container.sh; do
  if [[ ! -f "$repo_root/scripts/$helper" ]]; then
    echo "missing scripts/${helper}" >&2
    exit 127
  fi
done

container="${A3S_PG_CONTAINER:-$(bash "$repo_root/scripts/resolve-container.sh" a3s-postgres a3s-postgres)}"
admin_user="${A3S_PG_ADMIN_USER:-a3s_api}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required to prepare shared databases" >&2
  exit 1
fi

for _ in $(seq 1 60); do
  if docker exec "$container" pg_isready -U "$admin_user" -d postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! docker exec "$container" pg_isready -U "$admin_user" -d postgres >/dev/null 2>&1; then
  echo "shared Postgres ${container} is not ready" >&2
  exit 1
fi

psql_admin() {
  docker exec -i "$container" psql -U "$admin_user" -d postgres -v ON_ERROR_STOP=1 "$@"
}

# Preflight — host processes must authenticate to the compose Postgres itself.
# OrbStack forwards a guest's 127.0.0.1:5432 onto the Mac, so the probe uses
# the container bridge address when that address is reachable.
pg_host="$(bash "$repo_root/scripts/compose-postgres-host.sh")"
if [[ "$pg_host" == "127.0.0.1" ]]; then
  loopback_owner="$(lsof -nP -iTCP:5432 -sTCP:LISTEN 2>/dev/null | awk '$9=="127.0.0.1:5432" {print $1; exit}')"
  if [[ "$loopback_owner" == "OrbStack" ]]; then
    {
      echo ""
      echo "FATAL: 127.0.0.1:5432 is OrbStack, and ${container} is not reachable on its bridge address."
      echo "  Host services would authenticate against the guest database."
      echo "  Re-run after 'docker compose up -d a3s-postgres' can publish :5432."
      echo ""
    } >&2
    exit 1
  fi
fi
if command -v python3 >/dev/null 2>&1; then
  host_probe="$(python3 "$repo_root/scripts/host-pg-probe.py" \
    "${A3S_PG_HOST_PROBE_USER:-a3s_api}" \
    "${A3S_PG_HOST_PROBE_PASSWORD:-a3s_api}" \
    "$pg_host")"
  if [[ "$host_probe" != "ok" ]]; then
    {
      echo ""
      echo "FATAL: ${pg_host}:5432 did not accept the Postgres credentials (probe: ${host_probe})."
      echo "  (Override with A3S_PG_HOST_PROBE_USER / A3S_PG_HOST_PROBE_PASSWORD.)"
      echo ""
    } >&2
    exit 1
  fi
  echo "host Postgres preflight ok (${pg_host}:5432 -> ${container})"
fi

ensure_role() {
  local role="$1"
  local password="$2"
  psql_admin -c "DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${role}') THEN
    CREATE ROLE ${role} LOGIN PASSWORD '${password}';
  ELSE
    ALTER ROLE ${role} WITH LOGIN PASSWORD '${password}';
  END IF;
END
\$\$;"
}

ensure_database() {
  local database="$1"
  local owner="$2"
  if ! psql_admin -tAc "SELECT 1 FROM pg_database WHERE datname='${database}'" | grep -q 1; then
    psql_admin -c "CREATE DATABASE ${database} OWNER ${owner}"
  else
    psql_admin -c "ALTER DATABASE ${database} OWNER TO ${owner}"
  fi
  psql_admin -c "GRANT ALL PRIVILEGES ON DATABASE ${database} TO ${owner}"
}

ensure_role a3s_cloud a3s_cloud
ensure_role a3s_cloud_serving a3s_cloud_serving
ensure_role a3s_auth a3s_auth
ensure_role a3s_billing a3s_billing
ensure_role a3s_config a3s_config

ensure_database a3s_cloud a3s_cloud
ensure_database a3s_auth a3s_auth
ensure_database a3s_billing a3s_billing
ensure_database a3s_config a3s_config

psql_admin -c "GRANT CONNECT ON DATABASE a3s_cloud TO a3s_cloud_serving"

echo "shared Postgres ready: a3s_api, a3s_cloud, a3s_auth, a3s_billing, a3s_config on :5432"
