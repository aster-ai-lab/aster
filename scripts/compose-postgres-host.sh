#!/usr/bin/env bash
# Print the address host processes should use to reach aster-postgres.
# OrbStack forwards a guest's 127.0.0.1:5432 onto the Mac loopback, and that
# socket wins over the compose publish. The container bridge address still
# reaches the compose Postgres. Falls back to 127.0.0.1 when the container is down.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
container="${ASTER_PG_CONTAINER:-$(bash "$script_dir/resolve-container.sh" aster-postgres aster-postgres)}"
ip="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$container" 2>/dev/null || true)"
if [[ -n "${ip}" ]] && nc -z -G 1 "$ip" 5432 >/dev/null 2>&1; then
  printf '%s\n' "$ip"
  exit 0
fi
printf '%s\n' "127.0.0.1"
