#!/usr/bin/env bash
# Snapshot the named Docker volumes that carry OS development state.
# OrbStack's daemon has reset multiple times in a day, wiping containers
# while host-side volumes survived — this script adds an explicit,
# restorable copy so a volume wipe is also survivable.
#
# Usage: scripts/backup-volumes.sh [backup-dir]
# Restore pattern:
#   docker run --rm -v <volume>:/target -v <backup-dir>:/backup alpine \
#     tar xzf /backup/<volume>.tar.gz -C /target
set -euo pipefail

backup_dir="${1:-$HOME/.aster-backups}"
mkdir -p "$backup_dir"
stamp="$(date +%Y%m%d-%H%M%S)"

volumes=(
  api_aster_api_pg
  api_aster_api_redis
  api_aster_api_rustfs
  api_aster_api_clickhouse
  aster_cloud_registry
  aster_oci_registry
  kense_etcd
)

# ClickHouse caveat: a live tar of its volume can race active writes.
# The dev snapshot is best-effort; authoritative ClickHouse backups should
# use its BACKUP statement when that store carries state worth keeping.

for volume in "${volumes[@]}"; do
  if ! docker volume inspect "$volume" >/dev/null 2>&1; then
    echo "skip (absent): $volume"
    continue
  fi
  out="$backup_dir/$volume.$stamp.tar.gz"
  docker run --rm -v "$volume:/data:ro" -v "$backup_dir:/backup" alpine \
    tar czf "/backup/$(basename "$out")" -C /data .
  echo "backed up: $volume -> $out"
done

# Retain the 7 most recent snapshots per volume; prune older ones.
for volume in "${volumes[@]}"; do
  ls -t "$backup_dir/$volume".*.tar.gz 2>/dev/null | tail -n +8 | xargs rm -f 2>/dev/null || true
done
echo "backup round complete: $backup_dir"
