#!/bin/bash
# G0 only. Confirm the isolated ToWork lane is present, then print the launch
# environment. This script does not start a process, bind a port, or connect
# the toyou single-instance socket.
set -u
slug="towork-cua"
root="$(cd "$(dirname "$0")/../../.." && pwd)"
tenant="$root/apps/web/public/tenant/$slug"
lock="${TMPDIR:-/tmp}/dev.a3s.studio.toyou.lock"

if [[ "${A3S_TENANT_SLUG:-$slug}" == "toyou" ]]; then
  echo "refused slug toyou" >&2
  exit 2
fi

missing=0
for name in INFO.md logo-light.png mark-light.png icon.png icon.icns; do
  if [[ ! -f "$tenant/$name" ]]; then
    echo "missing $tenant/$name" >&2
    missing=1
  fi
done
if [[ -f "$tenant/model-market.json" ]]; then
  echo "refused model market in $tenant" >&2
  missing=1
fi
if [[ "$missing" != 0 ]]; then
  exit 1
fi

python3 - "$tenant/INFO.md" <<'PY'
import pathlib, sys
text = pathlib.Path(sys.argv[1]).read_text()
start = text.index("---\n") + 4
end = text.index("\n---", start)
front = text[start:end]
product = next(line.split(":", 1)[1].strip().strip('"') for line in front.splitlines() if line.startswith("product:"))
slug = next(line.split(":", 1)[1].strip().strip('"') for line in front.splitlines() if line.startswith("slug:"))
if product != "ToWork" or slug != "towork-cua":
    raise SystemExit(f"refused brand {slug} {product}")
print(f"brand {slug} {product}")
PY

echo "toyou_lock $lock"
if [[ -e "$lock" ]]; then
  lsof -nP "$lock" || true
else
  echo "toyou_lock absent"
fi
echo "listen_4318"
lsof -nP -iTCP:4318 -sTCP:LISTEN || true
echo "g0_launch"
echo "A3S_TENANT_SLUG=$slug cargo run --manifest-path apps/studio/src-tauri/Cargo.toml"
echo "data_dir $HOME/Library/Application Support/dev.a3s.studio/$slug"
echo "bundle_id dev.a3s.studio.$slug"
echo "g0_ready"
