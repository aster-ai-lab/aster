#!/usr/bin/env bash
# Pack the two aster-cli release binaries into one archive.
# Usage: scripts/package-aster-cli.sh <macos|linux|windows> <bin-dir> <out-file>
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: scripts/package-aster-cli.sh <macos|linux|windows> <bin-dir> <out-file>" >&2
  exit 2
fi

platform="$1"
bin_dir="$2"
out="$3"
stage="$(mktemp -d)"
cleanup() { rm -rf "$stage"; }
trap cleanup EXIT

copy_file() {
  local src="$1"
  local dest="$2"
  if [[ ! -f "$src" ]]; then
    echo "error: missing ${src}" >&2
    exit 1
  fi
  cp -f "$src" "$dest"
}

case "$platform" in
  macos|linux) suffix="" ;;
  windows) suffix=".exe" ;;
  *) echo "error: unsupported platform: $platform" >&2; exit 1 ;;
esac
copy_file "${bin_dir}/aster${suffix}" "${stage}/aster${suffix}"
copy_file "${bin_dir}/a3s-code-acp${suffix}" "${stage}/a3s-code-acp${suffix}"
chmod 755 "${stage}/aster${suffix}" "${stage}/a3s-code-acp${suffix}"
aster_identity="$("${stage}/aster${suffix}" --build-id)"
acp_identity="$("${stage}/a3s-code-acp${suffix}" --build-id)"
if [[ "$aster_identity" != aster-cli-pair-v1:* || "$aster_identity" != "$acp_identity" ]]; then
  echo "error: aster and a3s-code-acp must have the same source build identity" >&2
  exit 1
fi
printf '%s\n' "$aster_identity" > "${stage}/BUILD_ID"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$stage" && sha256sum "aster${suffix}" "a3s-code-acp${suffix}") > "${stage}/SHA256SUMS"
else
  (cd "$stage" && shasum -a 256 "aster${suffix}" "a3s-code-acp${suffix}") > "${stage}/SHA256SUMS"
fi
case "$platform" in
  macos|linux)
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$out" aster a3s-code-acp BUILD_ID SHA256SUMS
    ;;
  windows)
    if command -v python3 >/dev/null 2>&1; then
      py=(python3)
    elif command -v python >/dev/null 2>&1; then
      py=(python)
    else
      echo "error: packaging the Windows archive needs python" >&2
      exit 1
    fi
    "${py[@]}" - "$stage" "$out" <<'PY'
import sys, zipfile
from pathlib import Path
stage, out = Path(sys.argv[1]), Path(sys.argv[2])
with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name in ("aster.exe", "a3s-code-acp.exe", "BUILD_ID", "SHA256SUMS"):
        archive.write(stage / name, name)
PY
    ;;
  *)
    echo "error: unsupported platform: ${platform}" >&2
    exit 1
    ;;
esac
