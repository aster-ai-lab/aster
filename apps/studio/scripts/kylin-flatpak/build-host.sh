#!/usr/bin/env bash
# Build the Studio host and both companions from the root workspace for x86-64 Linux.
set -euo pipefail
cd "$(dirname "$0")/../.."
studio_root="$PWD"
workspace_root="$(cd ../.. && pwd)"
image="${KYLIN_BUILD_IMAGE:-aster-kylin-build:24.04}"
export A3S_TENANT_SLUG="${A3S_TENANT_SLUG-aster}"
export STUDIO_RENDERER_DIST="${STUDIO_RENDERER_DIST:-.studio-package}"
python3 scripts/studio_product.py "$A3S_TENANT_SLUG" product >/dev/null
export OFFICE_KERNEL_REQUIRED=1
bash ../../packages/office/scripts/build-office-kernel.sh
(cd ../../packages/office && ./node_modules/.bin/rslib)
node node_modules/@rsbuild/core/bin/rsbuild.js build
test -s "$STUDIO_RENDERER_DIST/windows/main/index.html"
mkdir -p dist/kylin
install -m644 ../../packages/office/node_modules/@embedpdf/fonts-sc/fonts/NotoSansHans-Regular.otf dist/kylin/NotoSansHans-Regular.otf
install -m644 ../../packages/office/node_modules/@embedpdf/fonts-sc/LICENSE dist/kylin/font-LICENSE
: >dist/kylin/package.env
docker run --rm --platform linux/amd64 \
  -v "$workspace_root:/aster:rw" \
  -v "$studio_root/dist/kylin/package.env:/aster/apps/studio/.env:ro" \
  -e CARGO_TARGET_DIR=/aster/target/studio-kylin-x86_64 \
  -e CARGO_BUILD_JOBS="${KYLIN_CARGO_JOBS:-2}" -e RUSTFLAGS='-C target-cpu=x86-64' \
  -e A3S_TENANT_SLUG -e STUDIO_RENDERER_DIST -w /aster "$image" bash -euo pipefail -c '
export PATH=/opt/rust-toolchain/bin:$PATH
export TAURI_CONFIG="$(python3 apps/studio/scripts/studio_product.py "$A3S_TENANT_SLUG" tauri-config --frontend-dist "../$STUDIO_RENDERER_DIST")"
feature="$(python3 apps/studio/scripts/studio_product.py "$A3S_TENANT_SLUG" feature)"
cargo build --locked --release -p a3s-code-acp -p a3s-office-cli
cargo build --locked --release -p a3s-studio --no-default-features --features "custom-protocol,$feature"
cd apps/studio
for name in Studio a3s-code-acp a3s-office; do
  install -m755 "$CARGO_TARGET_DIR/release/$name" "dist/kylin/$name"
  strip --strip-unneeded "dist/kylin/$name"
done
mkdir -p dist/kylin/native-libs dist/kylin/native-licenses
for name in libayatana-appindicator3.so.1 libayatana-indicator3.so.7 libayatana-ido3-0.4.so.0 libdbusmenu-gtk3.so.4 libdbusmenu-glib.so.4; do
  install -m644 "/usr/lib/x86_64-linux-gnu/$name" "dist/kylin/native-libs/$name"
done
for name in libayatana-appindicator3-1 libayatana-indicator3-7 libayatana-ido3-0.4-0 libdbusmenu-gtk3-4 libdbusmenu-glib4; do
  install -Dm644 "/usr/share/doc/$name/copyright" "dist/kylin/native-licenses/$name/copyright"
done
gdk-pixbuf-thumbnailer -s 128 "../web/public/tenant/$A3S_TENANT_SLUG/icon.png" dist/kylin/icon-128.png
python3 scripts/kylin-flatpak/delivery.py record-build dist/kylin "$STUDIO_RENDERER_DIST" \
  "$A3S_TENANT_SLUG" "$(python3 -c "import json;print(json.load(open(\"src-tauri/tauri.conf.json\"))[\"version\"])")" \
  "$(git rev-parse HEAD)"
'
