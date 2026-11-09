#!/usr/bin/env bash
# Build aster-cli Linux archives on an OrbStack machine via ubuntu:20.04.
#
# Usage:
#   scripts/orbstack-build-aster-cli-linux.sh <orb-machine> <linux|linux-arm64> <out-tar.gz>
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: $0 <orb-machine> <linux|linux-arm64> <out-tar.gz>" >&2
  exit 2
fi

machine="$1"
platform="$2"
out="$3"
case "$platform" in
  linux|linux-arm64) ;;
  *) echo "error: platform must be linux or linux-arm64" >&2; exit 2 ;;
esac

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
mkdir -p "${repo_root}/dist"
out_name="$(basename "$out")"
chmod +x "${script_dir}/orbstack-linux-release-inner.sh" \
  "${script_dir}/orbstack-linux-release-container.sh"

orb -m "$machine" bash "${repo_root}/scripts/orbstack-linux-release-inner.sh" \
  "$platform" "$out_name" "$repo_root"

if [[ ! -f "${repo_root}/dist/${out_name}" ]]; then
  echo "error: expected ${repo_root}/dist/${out_name}" >&2
  exit 1
fi
if [[ "$(cd "$(dirname "$out")" && pwd)/$(basename "$out")" != "${repo_root}/dist/${out_name}" ]]; then
  cp -f "${repo_root}/dist/${out_name}" "$out"
fi
echo "wrote ${repo_root}/dist/${out_name}"
