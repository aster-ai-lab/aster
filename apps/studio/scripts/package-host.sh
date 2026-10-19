#!/usr/bin/env bash
# Production host package. The renderer is the same rsbuild graph as `just studio`.
set -euo pipefail
cd "$(dirname "$0")/.."
target_dir="$(cargo metadata --locked --offline --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",x=>s+=x).on("end",()=>process.stdout.write(JSON.parse(s).target_directory))')"
export CARGO_TARGET_DIR="$target_dir"

export A3S_TENANT_SLUG="${A3S_TENANT_SLUG:-toyou}"
dist="${STUDIO_RENDERER_DIST:-.studio-package}"
export STUDIO_RENDERER_DIST="$dist"

if ! PYTHONPATH="$PWD/.py-fonttools" python3 -c 'import fontTools' >/dev/null 2>&1; then
  python3 -m pip install --disable-pip-version-check --target "$PWD/.py-fonttools" 'fonttools>=4.55'
fi

export OFFICE_KERNEL_REQUIRED=1
pnpm build:office
node node_modules/@rsbuild/core/bin/rsbuild.js build
PYTHONPATH="$PWD/.py-fonttools" python3 scripts/subset-layout-font.py "$dist"

cargo build --locked --release -p a3s-code-acp -p a3s-office-cli
./node_modules/.bin/tauri build --features custom-protocol \
  --config "{\"build\":{\"frontendDist\":\"../${dist}\"},\"bundle\":{\"macOS\":{\"signingIdentity\":\"-\"}}}"

app="$(find "$target_dir/release/bundle/macos" -name '*.app' -print | head -1)"
test -n "$app"
rm -rf "$app/Contents/Resources/studio-frontend"
mkdir -p "$app/Contents/Resources/studio-frontend"
cp -R "$dist"/. "$app/Contents/Resources/studio-frontend/"
test -f "$app/Contents/Resources/studio-frontend/windows/main/index.html"
echo "studio-package $app"
