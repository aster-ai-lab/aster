#!/usr/bin/env bash
# Run in an x86-64 Linux packaging container with GNOME 50 installed. Never installs the app.
set -euo pipefail
cd "$(dirname "$0")/../.."
runtime=org.gnome.Platform
branch=50
work="${STUDIO_KYLIN_WORK:-dist/kylin/flatpak-build}"
repo="${STUDIO_KYLIN_REPO:-dist/kylin/flatpak-repo}"
bundle="${STUDIO_KYLIN_BUNDLE:-dist/kylin/dev.a3s.studio.flatpak}"
tenant_root="${STUDIO_TENANT_ROOT:-../web/public/tenant}"
renderer="${STUDIO_RENDERER_DIST:-.studio-package}"
inputs="${STUDIO_KYLIN_INPUTS:-dist/kylin}"
im_assets="${STUDIO_KYLIN_IM_ASSETS:-$inputs}"
test ! -e "$work" || { echo "refusing existing work directory: $work" >&2; exit 1; }
flatpak info --user "$runtime//$branch" >/dev/null
flatpak build-init "$work" dev.a3s.studio "$runtime" "$runtime" "$branch"
python3 scripts/kylin-flatpak/delivery.py stage "$inputs" "$renderer" "$tenant_root" "$im_assets" "$work/files"
for binary in /app/bin/Studio /app/bin/agent/a3s-code-acp /app/bin/a3s-office /app/bin/kylin-im-backend \
  /app/lib/im/fcitx4/im-fcitx.so /app/lib/im/fcitx5/im-fcitx5.so; do
  links="$(flatpak build "$work" ldd "$binary")"
  if grep -F 'not found' <<<"$links"; then echo "missing runtime library: $binary" >&2; exit 1; fi
done
for module in /app/lib/im/fcitx4/im-fcitx.so /app/lib/im/fcitx5/im-fcitx5.so; do
  flatpak build "$work" python3 -I -c 'import ctypes,os,sys;ctypes.CDLL(sys.argv[1],mode=os.RTLD_NOW)' "$module"
done
flatpak build "$work" gtk-query-immodules-3.0 \
  | grep -F '"ibus"' >/dev/null || { echo 'runtime has no ibus cache entry' >&2; exit 1; }
flatpak build-finish "$work" --command=kylin-launch --share=network --share=ipc \
  --socket=fallback-x11 --socket=wayland --socket=pulseaudio --device=dri \
  --filesystem=home --filesystem=xdg-download --filesystem=xdg-data/fonts:ro \
  --filesystem=xdg-config/fontconfig:ro --talk-name=org.freedesktop.portal.Desktop \
  --talk-name=org.freedesktop.portal.Documents --talk-name=org.freedesktop.portal.FileChooser \
  --talk-name=org.freedesktop.portal.Fcitx --talk-name=org.freedesktop.Notifications \
  --talk-name='org.fcitx.*' --talk-name=org.freedesktop.IBus
runtime_dir="$(flatpak info --user --show-location "$runtime//$branch")/files"
bash scripts/kylin-flatpak/test-input-method.sh "$work/files" "$runtime_dir"
python3 scripts/kylin-flatpak/delivery.py verify "$work/files"
mkdir -p "$repo" "$(dirname "$bundle")"
flatpak build-export "$repo" "$work" "$branch"
flatpak build-bundle "$repo" "$bundle" dev.a3s.studio "$branch"
sha256sum "$bundle" >"$bundle.sha256"
echo "studio-kylin-bundle $bundle"
