#!/usr/bin/env bash
# Stop OrbStack containers that belong to the local A3S Cloud stack but are
# outside the pid file cloud_down.sh already handles.
set -euo pipefail

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is unavailable; skipped cloud containers"
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
docker compose -f "$repo_root/docker-compose.yml" stop a3s-cloud-registry >/dev/null 2>&1 || true

for name in \
  \
  a3s-cloud-registry \
  \
  a3s-ga2-pg \
  a3s-cloud-postgres \
  a3s-cloud-nats \
  a3s-cloud-registry \
  a3s-cloud-dev-postgres \
  a3s-cloud-dev-nats \
  a3s-cloud-dev-registry
do
  if docker ps -a --format '{{.Names}}' | grep -qx "$name"; then
    echo "stopping docker container $name"
    docker stop "$name" >/dev/null 2>&1 || true
  fi
done

echo "a3s cloud containers stopped"
