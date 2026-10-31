#!/usr/bin/env bash
# Stop OrbStack containers that belong to the local Aster Cloud stack but are
# outside the pid file cloud_down.sh already handles.
set -euo pipefail

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is unavailable; skipped cloud containers"
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
docker compose -f "$repo_root/docker-compose.yml" stop aster-cloud-registry >/dev/null 2>&1 || true

for name in \
  \
  aster-cloud-registry \
  \
  aster-ga2-pg \
  aster-cloud-postgres \
  aster-cloud-nats \
  aster-cloud-registry \
  aster-cloud-dev-postgres \
  aster-cloud-dev-nats \
  aster-cloud-dev-registry
do
  if docker ps -a --format '{{.Names}}' | grep -qx "$name"; then
    echo "stopping docker container $name"
    docker stop "$name" >/dev/null 2>&1 || true
  fi
done

echo "aster cloud containers stopped"
