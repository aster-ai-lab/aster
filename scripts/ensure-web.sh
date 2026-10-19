#!/usr/bin/env bash
# Ensure the Console dev server is listening on :3100.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$repo_root/scripts/detach-service.sh"
cd "$repo_root"

state_dir="apps/web/.data"
mkdir -p "$state_dir"
pid_file="$state_dir/web.pid"
log_file="$state_dir/web.log"
stamp_file="$state_dir/tenant.stamp"

slug="${A3S_TENANT_SLUG:-}"
if [[ -z "$slug" ]] || ! [[ "$slug" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
  echo "invalid tenant slug: ${slug}" >&2
  exit 1
fi
export A3S_TENANT_SLUG="$slug"
export PUBLIC_A3S_TENANT_SLUG="$slug"
hash_line="$(bun "$repo_root/scripts/check-tenant.ts" "$slug")"
if ! [[ "$hash_line" =~ ^[0-9a-f]{64}$ ]]; then
  echo "tenant check returned an unexpected digest for ${slug}" >&2
  exit 1
fi

health_ok() {
  curl -sf --max-time 2 -o /dev/null "http://127.0.0.1:3100/"
}

ensure_ssh_bridge() {
  if curl -sf --max-time 1 -o /dev/null "http://127.0.0.1:13101/"; then
    return 0
  fi
  local saved_pid="$pid_file"
  local saved_log="$log_file"
  pid_file="$state_dir/fleet-ssh.pid"
  log_file="$state_dir/fleet-ssh.log"
  launch_service bun "$repo_root/apps/web/src/server/fleetSshBridge.ts"
  pid_file="$saved_pid"
  log_file="$saved_log"
}

stamp_matches=0
if [[ -f "$stamp_file" ]] && printf '%s\n%s\n' "$slug" "$hash_line" | cmp -s - "$stamp_file"; then
  stamp_matches=1
fi

if health_ok && [[ "$stamp_matches" -eq 1 ]]; then
  service_pid_alive || remember_listener 3100
  ensure_ssh_bridge
  echo "a3s console already answering on :3100"
  exit 0
fi

if health_ok; then
  echo "tenant stamp changed, restarting console"
fi

# Rsbuild's default host is localhost, which on this machine is ::1 only.
# stop-web clears that listener before a 127.0.0.1 bind.
# stop-web does not stop the fleet SSH bridge on :13101.
bash "$repo_root/scripts/stop-web.sh"

echo "starting a3s console on :3100 (tenant=${slug})…"
if [[ ! -x "$repo_root/apps/web/node_modules/.bin/rsbuild" && ! -x "$repo_root/node_modules/.bin/rsbuild" ]]; then
  echo "installing console dependencies…"
  bun install
fi
launch_service bun --filter @a3s/web dev
ensure_ssh_bridge

for _ in $(seq 1 80); do
  if health_ok; then
    printf '%s\n%s\n' "$slug" "$hash_line" > "$stamp_file"
    echo "a3s console ready  (pid $(cat "$pid_file") · log $log_file)"
    exit 0
  fi
  sleep 0.5
done

echo "a3s console failed to answer on :3100 — see $log_file" >&2
exit 1
