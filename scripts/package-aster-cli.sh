#!/usr/bin/env bash
# Pack the two aster-cli release binaries into one archive.
# Usage: scripts/package-aster-cli.sh <macos|linux|kylin|windows> <bin-dir> <out-file>
#
# `kylin` is the Kylin V10 SP1 target: x86_64, glibc 2.31. It refuses binaries
# that need a newer glibc, because the archive would fail on the target machine
# with a missing GLIBC_2.3x symbol rather than at packaging time.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: scripts/package-aster-cli.sh <macos|linux|kylin|windows> <bin-dir> <out-file>" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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

max_glibc=""
case "$platform" in
  macos|linux) suffix="" ;;
  kylin)
    suffix=""
    max_glibc="2.31"
    ;;
  windows) suffix=".exe" ;;
  *) echo "error: unsupported platform: $platform" >&2; exit 1 ;;
esac
copy_file "${bin_dir}/aster${suffix}" "${stage}/aster${suffix}"
copy_file "${bin_dir}/a3s-code-acp${suffix}" "${stage}/a3s-code-acp${suffix}"
chmod 755 "${stage}/aster${suffix}" "${stage}/a3s-code-acp${suffix}"
release_files=("aster${suffix}" "a3s-code-acp${suffix}")
if [[ "$platform" == macos ]]; then
  members="$(python3 "${script_dir}/aster_sandbox.py" members --directory "${bin_dir}/sandbox")"
  while IFS= read -r name; do
    release_files+=("$name")
    mkdir -p "${stage}/$(dirname "$name")"
    copy_file "${bin_dir}/${name}" "${stage}/${name}"
  done <<< "$members"
  python3 "${script_dir}/aster_sandbox.py" verify --directory "${stage}/sandbox"
fi
if [[ -n "$max_glibc" ]]; then
  if command -v python3 >/dev/null 2>&1; then
    py=(python3)
  elif command -v python >/dev/null 2>&1; then
    py=(python)
  else
    echo "error: checking the ${platform} glibc baseline needs python" >&2
    exit 1
  fi
  "${py[@]}" "${script_dir}/check-glibc-symbols.py" --max "$max_glibc" \
    "${stage}/aster${suffix}" "${stage}/a3s-code-acp${suffix}"
fi
aster_identity="$("${stage}/aster${suffix}" --build-id)"
acp_identity="$("${stage}/a3s-code-acp${suffix}" --build-id)"
if [[ "$aster_identity" != aster-cli-pair-v1:* || "$aster_identity" != "$acp_identity" ]]; then
  echo "error: aster and a3s-code-acp must have the same source build identity" >&2
  exit 1
fi
printf '%s\n' "$aster_identity" > "${stage}/BUILD_ID"
sha256_file() {
  local result
  if command -v sha256sum >/dev/null 2>&1; then result="$(sha256sum -b "$1")";
  else result="$(shasum -a 256 -b "$1")"; fi
  printf '%s  %s\n' "${result%% *}" "$1"
}
(
  cd "$stage"
  for name in "${release_files[@]}"; do sha256_file "$name"; done
) > "${stage}/SHA256SUMS"
case "$platform" in
  macos|linux|kylin)
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$out" "${release_files[@]}" BUILD_ID SHA256SUMS
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
