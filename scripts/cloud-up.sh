#!/usr/bin/env bash
# Bring up the Aster Cloud control-plane API against the root compose stack.
#
# apps/cloud/tools/dev/run_cloud.sh defaults to an aster-box compose
# dependency and an object endpoint (host.orb.internal) that only routes
# from inside OrbStack. This wrapper runs the same build → migrate → serve
# flow against the shared compose stack instead: Postgres from
# scripts/ensure-shared-databases.sh and objects from the compose RustFS on
# loopback. It derives a local ACL (still the sole product configuration
# format) that differs from config/cloud.acl only in the object endpoint;
# every environment value stays overridable and the caller (Tiltfile)
# tolerates failure by design.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cloud_root="$repo_root/apps/cloud"
state_dir="$cloud_root/.aster"
local_acl="$state_dir/cloud-host.acl"

if ! docker compose ps --status running aster-postgres 2>/dev/null | grep -q aster-postgres; then
  echo "shared Postgres is not running; start it with:" >&2
  echo "  docker compose up -d aster-postgres && bash scripts/ensure-shared-databases.sh" >&2
  exit 1
fi
pg_host="$(bash scripts/compose-postgres-host.sh)"

# Run against this checkout's OWN database. The shared aster_cloud database may
# be owned by another running control plane (another checkout); sharing it
# corrupts reconciler state across schemas. create-if-missing is idempotent.
cloud_database="${ASTER_CLOUD_DATABASE:-aster_cloud_aster}"
container="${ASTER_PG_CONTAINER:-$(bash scripts/resolve-container.sh aster-postgres aster-postgres)}"
docker exec "$container" psql -U aster_api -d postgres -tAc \
  "select 1 from pg_database where datname = '$cloud_database'" | grep -q 1 \
  || docker exec "$container" psql -U aster_api -d postgres -c \
    "create database $cloud_database owner aster_cloud" >/dev/null

# The dev ACL's object storage credentials read from these environment
# names; default them to the compose RustFS values.
export ASTER_CLOUD_S3_ACCESS_KEY_ID="${ASTER_CLOUD_S3_ACCESS_KEY_ID:-kense}"
export ASTER_CLOUD_S3_SECRET_ACCESS_KEY="${ASTER_CLOUD_S3_SECRET_ACCESS_KEY:-aster-rustfs-dev-secret}"
object_endpoint="${ASTER_CLOUD_OBJECT_ENDPOINT:-http://127.0.0.1:9000}"

# The control plane writes a startup health object into its bucket; create it
# best-effort when python3+boto3 are available, otherwise say what is missing.
if python3 - >/dev/null 2>&1 <<PY
import boto3
s3 = boto3.client(
    "s3",
    endpoint_url="$object_endpoint",
    aws_access_key_id="$ASTER_CLOUD_S3_ACCESS_KEY_ID",
    aws_secret_access_key="$ASTER_CLOUD_S3_SECRET_ACCESS_KEY",
    region_name="us-east-1",
)
names = [bucket["Name"] for bucket in s3.list_buckets()["Buckets"]]
if "aster-cloud-objects" not in names:
    s3.create_bucket(Bucket="aster-cloud-objects")
PY
then
  :
else
  echo "note: could not ensure the aster-cloud-objects bucket in RustFS" \
    "(python3 with boto3 unavailable); create it manually if boot fails" >&2
fi

mkdir -p "$state_dir"
api_port="${ASTER_CLOUD_API_PORT:-8080}"
node_control_port="${ASTER_CLOUD_NODE_CONTROL_PORT:-8443}"
sed \
  -e "s#^  endpoint = \"http://host.orb.internal:9000\"#  endpoint = \"$object_endpoint\"#" \
  -e "s#^  port = 8080#  port = $api_port#" \
  -e "s#^  port = 8443#  port = $node_control_port#" \
  "$cloud_root/config/cloud.acl" > "$local_acl"

generated_bootstrap_token=false
if [[ -z ${ASTER_CLOUD_BOOTSTRAP_TOKEN:-} ]]; then
  export ASTER_CLOUD_BOOTSTRAP_TOKEN="$(openssl rand -hex 32)"
  generated_bootstrap_token=true
fi
if [[ -z ${ASTER_CLOUD_GITHUB_WEBHOOK_SECRET:-} ]]; then
  export ASTER_CLOUD_GITHUB_WEBHOOK_SECRET="$(openssl rand -hex 32)"
fi

target_directory="${CARGO_TARGET_DIR:-$cloud_root/target}"
if [[ $target_directory != /* ]]; then
  target_directory="$repo_root/$target_directory"
fi
(cd "$cloud_root" && cargo build --locked -p aster-cloud-control-plane)
api_bin="$target_directory/debug/aster-cloud-control-plane"
migration_bin="$target_directory/debug/aster-cloud-migrate"

export ASTER_CLOUD_POSTGRES_URL="${ASTER_CLOUD_POSTGRES_URL:-postgres://aster_cloud:aster_cloud@${pg_host}:5432/${cloud_database}}"
ASTER_CLOUD_POSTGRES_MIGRATION_URL="${ASTER_CLOUD_POSTGRES_MIGRATION_URL:-$ASTER_CLOUD_POSTGRES_URL}" \
  "$migration_bin" "$local_acl"

printf '%s\n' \
  'Aster Cloud control-plane API is starting:' \
  "  API: http://127.0.0.1:${api_port}/api/v1" \
  "  config: $local_acl (derived from config/cloud.acl; object endpoint $object_endpoint, api :${api_port}, node-control :${node_control_port})"
if [[ $generated_bootstrap_token == true ]]; then
  printf '  Bootstrap token for this run: %s\n' "$ASTER_CLOUD_BOOTSTRAP_TOKEN"
fi
exec "$api_bin" "$local_acl"
