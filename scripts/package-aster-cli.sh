#!/usr/bin/env bash
# Pack the aster-cli release binaries and embedding assets into one archive.
# Usage: scripts/package-aster-cli.sh <macos|linux|windows> <bin-dir> <out-file>
#
# `linux` targets x86_64, glibc 2.31. It refuses binaries that need a newer
# glibc, because the archive would fail on the target machine
# with a missing GLIBC_2.3x symbol rather than at packaging time.
#
# Embedding weights ship as `embedding/*` next to the binaries so `aster-acp`
# release builds can omit `bundled-assets` without a network fetch.
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: scripts/package-aster-cli.sh <macos|linux|windows> <bin-dir> <out-file>" >&2
  exit 2
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
embedding_src="${repo_root}/crates/embedding/assets"
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
  macos) suffix="" ;;
  linux)
    suffix=""
    max_glibc="2.31"
    ;;
  windows) suffix=".exe" ;;
  *) echo "error: unsupported platform: $platform" >&2; exit 1 ;;
esac
copy_file "${bin_dir}/aster${suffix}" "${stage}/aster${suffix}"
copy_file "${bin_dir}/aster-acp${suffix}" "${stage}/aster-acp${suffix}"
chmod 755 "${stage}/aster${suffix}" "${stage}/aster-acp${suffix}"
mkdir -p "${stage}/embedding"
copy_file "${embedding_src}/potion-code-16M-v2.safetensors" \
  "${stage}/embedding/potion-code-16M-v2.safetensors"
copy_file "${embedding_src}/potion-code-16M-v2.tokenizer.json" \
  "${stage}/embedding/potion-code-16M-v2.tokenizer.json"
copy_file "${embedding_src}/potion-code-16M-v2.config.json" \
  "${stage}/embedding/potion-code-16M-v2.config.json"
release_files=("aster${suffix}" "aster-acp${suffix}")
embedding_files=(
  "embedding/potion-code-16M-v2.safetensors"
  "embedding/potion-code-16M-v2.tokenizer.json"
  "embedding/potion-code-16M-v2.config.json"
)
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
    "${stage}/aster${suffix}" "${stage}/aster-acp${suffix}"
fi
aster_identity="$("${stage}/aster${suffix}" --build-id)"
acp_identity="$("${stage}/aster-acp${suffix}" --build-id)"
if [[ "$aster_identity" != aster-cli-pair-v1:* || "$aster_identity" != "$acp_identity" ]]; then
  echo "error: aster and aster-acp must have the same source build identity" >&2
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
  macos|linux)
    # File members only (no directory entries) so installers that refuse
    # non-regular members stay compatible.
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$out" \
      "${release_files[@]}" BUILD_ID SHA256SUMS "${embedding_files[@]}"
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
members = (
    "aster.exe",
    "aster-acp.exe",
    "BUILD_ID",
    "SHA256SUMS",
    "embedding/potion-code-16M-v2.safetensors",
    "embedding/potion-code-16M-v2.tokenizer.json",
    "embedding/potion-code-16M-v2.config.json",
)
with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name in members:
        archive.write(stage / name, name)
PY
    ;;
  *)
    echo "error: unsupported platform: ${platform}" >&2
    exit 1
    ;;
esac
