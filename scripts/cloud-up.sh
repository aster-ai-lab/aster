#!/usr/bin/env bash
# Bring up the A3S Cloud control-plane API against the root compose stack.
#
# apps/cloud/tools/dev/run_cloud.sh defaults to an a3s-box compose
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
state_dir="$cloud_root/.a3s"
local_acl="$state_dir/cloud-host.acl"

if ! docker compose ps --status running a3s-postgres 2>/dev/null | grep -q a3s-postgres; then
  echo "shared Postgres is not running; start it with:" >&2
  echo "  docker compose up -d a3s-postgres && bash scripts/ensure-shared-databases.sh" >&2
  exit 1
fi
pg_host="$(bash scripts/compose-postgres-host.sh)"

# Run against this checkout's OWN database. The shared a3s_cloud database may
# be owned by another running control plane (another checkout); sharing it
# corrupts reconciler state across schemas. create-if-missing is idempotent.
cloud_database="${A3S_CLOUD_DATABASE:-a3s_cloud_aster}"
container="${A3S_PG_CONTAINER:-$(bash scripts/resolve-container.sh a3s-postgres a3s-postgres)}"
docker exec "$container" psql -U a3s_api -d postgres -tAc \
  "select 1 from pg_database where datname = '$cloud_database'" | grep -q 1 \
  || docker exec "$container" psql -U a3s_api -d postgres -c \
    "create database $cloud_database owner a3s_cloud" >/dev/null

# The dev ACL's object storage credentials read from these environment
# names; default them to the compose RustFS values.
export A3S_CLOUD_S3_ACCESS_KEY_ID="${A3S_CLOUD_S3_ACCESS_KEY_ID:-kense}"
export A3S_CLOUD_S3_SECRET_ACCESS_KEY="${A3S_CLOUD_S3_SECRET_ACCESS_KEY:-a3s-rustfs-dev-secret}"
object_endpoint="${A3S_CLOUD_OBJECT_ENDPOINT:-http://127.0.0.1:9000}"

# The control plane writes a startup health object into its bucket; create it
# best-effort when python3+boto3 are available, otherwise say what is missing.
if python3 - >/dev/null 2>&1 <<PY
import boto3
s3 = boto3.client(
    "s3",
    endpoint_url="$object_endpoint",
    aws_access_key_id="$A3S_CLOUD_S3_ACCESS_KEY_ID",
    aws_secret_access_key="$A3S_CLOUD_S3_SECRET_ACCESS_KEY",
    region_name="us-east-1",
)
names = [bucket["Name"] for bucket in s3.list_buckets()["Buckets"]]
if "a3s-cloud-objects" not in names:
    s3.create_bucket(Bucket="a3s-cloud-objects")
PY
then
  :
else
  echo "note: could not ensure the a3s-cloud-objects bucket in RustFS" \
    "(python3 with boto3 unavailable); create it manually if boot fails" >&2
fi

mkdir -p "$state_dir"
api_port="${A3S_CLOUD_API_PORT:-8080}"
node_control_port="${A3S_CLOUD_NODE_CONTROL_PORT:-8443}"
sed \
  -e "s#^  endpoint = \"http://host.orb.internal:9000\"#  endpoint = \"$object_endpoint\"#" \
  -e "s#^  port = 8080#  port = $api_port#" \
  -e "s#^  port = 8443#  port = $node_control_port#" \
  "$cloud_root/config/cloud.acl" > "$local_acl"

generated_bootstrap_token=false
if [[ -z ${A3S_CLOUD_BOOTSTRAP_TOKEN:-} ]]; then
  export A3S_CLOUD_BOOTSTRAP_TOKEN="$(openssl rand -hex 32)"
  generated_bootstrap_token=true
fi
if [[ -z ${A3S_CLOUD_GITHUB_WEBHOOK_SECRET:-} ]]; then
  export A3S_CLOUD_GITHUB_WEBHOOK_SECRET="$(openssl rand -hex 32)"
fi

target_directory="${CARGO_TARGET_DIR:-$cloud_root/target}"
if [[ $target_directory != /* ]]; then
  target_directory="$repo_root/$target_directory"
fi
(cd "$cloud_root" && cargo build --locked -p a3s-cloud-control-plane)
api_bin="$target_directory/debug/a3s-cloud-control-plane"
migration_bin="$target_directory/debug/a3s-cloud-migrate"

export A3S_CLOUD_POSTGRES_URL="${A3S_CLOUD_POSTGRES_URL:-postgres://a3s_cloud:a3s_cloud@${pg_host}:5432/${cloud_database}}"
A3S_CLOUD_POSTGRES_MIGRATION_URL="${A3S_CLOUD_POSTGRES_MIGRATION_URL:-$A3S_CLOUD_POSTGRES_URL}" \
  "$migration_bin" "$local_acl"

printf '%s\n' \
  'A3S Cloud control-plane API is starting:' \
  "  API: http://127.0.0.1:${api_port}/api/v1" \
  "  config: $local_acl (derived from config/cloud.acl; object endpoint $object_endpoint, api :${api_port}, node-control :${node_control_port})"
if [[ $generated_bootstrap_token == true ]]; then
  printf '  Bootstrap token for this run: %s\n' "$A3S_CLOUD_BOOTSTRAP_TOKEN"
fi
exec "$api_bin" "$local_acl"
