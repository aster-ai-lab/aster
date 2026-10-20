#!/usr/bin/env bash
# Build the fcitx4 and fcitx5 GTK3 input-method modules for the Kylin Flatpak.
# The .so is built in the linux/amd64 packaging image, against that image's
# GTK3. package-bundle.sh then checks that the GNOME 50 runtime can load it.
# This does not build Studio and does not type into a Kylin desktop.
set -euo pipefail
cd "$(dirname "$0")/../.."

version="5.1.7"
sha256="92d7a5e3ed9eac8c8173983723013816f4706dddc3f019b4bb5c76c48cf4dd92"
url="https://github.com/fcitx/fcitx5-gtk/archive/refs/tags/${version}.tar.gz"
image="${KYLIN_BUILD_IMAGE:-aster-kylin-build:24.04}"
out="dist/kylin/immodules/fcitx5"

if [[ "${KYLIN_IMMODULE_INNER:-}" != 1 ]]; then
  docker run --rm --platform linux/amd64 \
    -v "$PWD:/src:rw" \
    -e KYLIN_IMMODULE_INNER=1 \
    -w /src \
    "$image" \
    bash scripts/kylin-flatpak/build-immodule.sh
  test -s "${out}/im-fcitx5.so"
  test -s "${out}/immodules.cache"
  test -s "${out}/module-id"
  test -s "${out}/query-path"
  test -s "${out}/LGPL-2.1-or-later.txt"
  echo "kylin-immodule ${out}/im-fcitx5.so"
  test -s dist/kylin/immodules/fcitx4/im-fcitx.so
  test -s dist/kylin/immodules/fcitx4/libfcitx-gclient.so.1
  test -s dist/kylin/immodules/fcitx4/libfcitx-utils.so.0
  test -x dist/kylin/kylin-im-backend
  exit 0
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  build-essential ca-certificates cmake curl extra-cmake-modules \
  g++ libdbus-1-dev libglib2.0-dev libgtk-3-bin libgtk-3-dev libxkbcommon-dev \
  libxkbcommon-x11-dev patchelf pkg-config python3

mkdir -p dist/kylin
cc -O2 -Wall -Wextra -Werror scripts/kylin-flatpak/kylin_im_backend.c \
  -o dist/kylin/kylin-im-backend $(pkg-config --cflags --libs gdk-3.0)
strip --strip-unneeded dist/kylin/kylin-im-backend

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "${work}/src.tar.gz" "$url"
echo "${sha256}  ${work}/src.tar.gz" | sha256sum -c -
tar -C "$work" -xzf "${work}/src.tar.gz"
src="${work}/fcitx5-gtk-${version}"
cmake -S "$src" -B "${work}/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX=/usr \
  -DENABLE_GIR=OFF \
  -DENABLE_GTK2_IM_MODULE=OFF \
  -DENABLE_GTK3_IM_MODULE=ON \
  -DENABLE_GTK4_IM_MODULE=OFF \
  -DENABLE_SNOOPER=OFF \
  -DBUILD_ONLY_PLUGIN=ON
cmake --build "${work}/build" --parallel
cmake --install "${work}/build" --prefix "${work}/install"
module="$(find "${work}/install" -name im-fcitx5.so -type f)"
test -n "$module"
install -D "$module" "/src/${out}/im-fcitx5.so"
strip --strip-unneeded "/src/${out}/im-fcitx5.so"
patchelf --remove-rpath "/src/${out}/im-fcitx5.so"
query_bin="$(command -v gtk-query-immodules-3.0 || true)"
if [[ -z "$query_bin" ]]; then
  query_bin="$(find /usr -name gtk-query-immodules-3.0 -type f -print -quit)"
fi
if [[ -z "$query_bin" ]]; then
  echo "gtk-query-immodules-3.0 is not in the packaging image" >&2
  exit 1
fi
query_path="/src/${out}/im-fcitx5.so"
"$query_bin" "$query_path" >"/src/${out}/immodules.cache"
python3 /src/scripts/kylin-flatpak/immodule_cache.py id \
  --cache "/src/${out}/immodules.cache" \
  --query-path "$query_path" >"/src/${out}/module-id"
printf '%s\n' "$query_path" >"/src/${out}/query-path"
install -Dm644 "${src}/LICENSES/LGPL-2.1-or-later.txt" \
  "/src/${out}/LGPL-2.1-or-later.txt"
file "/src/${out}/im-fcitx5.so"
echo "module-id=$(cat "/src/${out}/module-id")"
install -Dm644 "${work}/src.tar.gz" dist/kylin/immodules/fcitx5/fcitx5-gtk-5.1.7.tar.gz
bash scripts/kylin-flatpak/build-fcitx4-immodule.sh
python3 scripts/kylin-flatpak/delivery.py record-ime dist/kylin
