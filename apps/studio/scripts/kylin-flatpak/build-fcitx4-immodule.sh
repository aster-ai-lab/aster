#!/usr/bin/env bash
# Called inside build-immodule.sh's linux/amd64 container. Build only the
# GTK3 client and its two libraries; the host keeps its existing IME daemon.
set -euo pipefail
cd "$(dirname "$0")/../.."

version=4.2.9.9
sha256=307bddeabbec383eb27c4ea222d9f71d8d9011d7942f4010368e5bc3ca7440a1
out="$PWD/dist/kylin/immodules/fcitx4"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="$PWD/dist/kylin/dependencies/fcitx-${version}.tar.xz"
mkdir -p "$(dirname "$archive")"
if [[ ! -f "$archive" ]]; then
  curl -fL --connect-timeout 20 --max-time 300 --retry 2 \
    "https://download.fcitx-im.org/fcitx/fcitx-${version}.tar.xz" -o "$archive"
fi
printf '%s  %s\n' "$sha256" "$archive" | sha256sum -c -
tar -C "$work" -xJf "$archive"
src="${work}/fcitx-${version}"
patch -d "$src" -p1 --fuzz=0 <scripts/kylin-flatpak/fcitx4_legacy_portal.patch
cmake -S "$src" -B "${work}/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DENABLE_QT=OFF -DENABLE_X11=OFF -DENABLE_XKB=OFF \
  -DENABLE_GTK2_IM_MODULE=OFF -DENABLE_GTK3_IM_MODULE=ON \
  -DENABLE_GIR=OFF -DENABLE_SNOOPER=ON -DENABLE_GETTEXT=OFF \
  -DENABLE_OPENCC=OFF -DENABLE_ENCHANT=OFF -DENABLE_PRESAGE=OFF
cmake --build "${work}/build" --target im-fcitx-gtk3 --parallel
mkdir -p "$out"
install -m755 "${work}/build/src/frontend/gtk3/im-fcitx.so" "${out}/im-fcitx.so"
install -m755 "${work}/build/src/lib/fcitx-gclient/libfcitx-gclient.so.0.2" "${out}/libfcitx-gclient.so.1"
install -m755 "${work}/build/src/lib/fcitx-utils/libfcitx-utils.so.0.1" "${out}/libfcitx-utils.so.0"
for binary in "$out"/*.so*; do
  strip --strip-unneeded "$binary"
  patchelf --set-rpath '$ORIGIN' "$binary"
done
query_bin="$(find /usr -name gtk-query-immodules-3.0 -type f -print -quit)"
test -n "$query_bin"
"$query_bin" "${out}/im-fcitx.so" >"${out}/immodules.cache"
python3 scripts/kylin-flatpak/immodule_cache.py id \
  --cache "${out}/immodules.cache" --query-path "${out}/im-fcitx.so" >"${out}/module-id"
printf '%s\n' "${out}/im-fcitx.so" >"${out}/query-path"
install -m644 "${src}/COPYING" "${out}/COPYING"
# Ship the exact corresponding source with this GPL client module.
install -m644 "$archive" "${out}/fcitx-${version}.tar.xz"
install -m644 scripts/kylin-flatpak/fcitx4_legacy_portal.patch "$out/fcitx4_legacy_portal.patch"
echo "kylin-fcitx4-immodule ${out}/im-fcitx.so"
