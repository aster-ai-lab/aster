#!/usr/bin/env bash
# Install the aster-cli pair from this repository's GitHub Releases.
#
# One command:
#   curl -fsSL https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh | bash -s -- --prefix "$HOME/.local/bin"
#
# The newest release is discovered from the channel manifest:
#   https://github.com/aster-ai-lab/aster/releases/latest/download/latest.json
# `--version X.Y.Z` installs that exact release. A positional URL may name
# one official archive directly. macOS downloads a .tar.gz, Linux a .tar.gz,
# Windows a .zip; each archive contains aster and a3s-code-acp.
# A Linux host whose glibc predates 2.32 (Kylin V10 SP1, Ubuntu 20.04, Debian
# 11) installs the `kylin` archive, which is the same x86_64 pair built against
# glibc 2.31; the newer-glibc `linux` archive cannot load there.
# The command does not clone this repository and does not compile.
set -euo pipefail

official_releases="https://github.com/aster-ai-lab/aster/releases"
latest_manifest="${official_releases}/latest/download/latest.json"

usage() {
  echo "Usage: install.sh [--prefix DIR] [--version X.Y.Z] [ARCHIVE-URL]" >&2
  echo "https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh" >&2
  echo "${latest_manifest}" >&2
  exit 2
}

refuse() {
  echo "error: this installer does not download that binary" >&2
  exit 1
}

if [[ -n "${ASTER_INSTALL_URL:-}" || -n "${GROK_INSTALL_URL:-}" ]]; then
  refuse
fi

prefix=""
want_version=""
direct_url=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      shift
      [[ $# -gt 0 ]] || usage
      prefix="$1"
      ;;
    --prefix=*)
      prefix="${1#--prefix=}"
      ;;
    --version)
      shift
      [[ $# -gt 0 ]] || usage
      want_version="$1"
      ;;
    --version=*)
      want_version="${1#--version=}"
      ;;
    -h|--help)
      usage
      ;;
    *://*)
      if [[ "$1" =~ ^https://github\.com/aster-ai-lab/aster/releases/download/aster-cli-v[0-9]+\.[0-9]+\.[0-9]+/aster-cli-v[0-9]+\.[0-9]+\.[0-9]+-(macos|linux|kylin|windows)\.(tar\.gz|zip)$ ]]; then
        direct_url="$1"
      else
        refuse
      fi
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage
      ;;
  esac
  shift
done

case "$prefix" in
  *://*) refuse ;;
esac

if [[ -n "$want_version" ]]; then
  want_version="${want_version#aster-cli-v}"
  want_version="${want_version#v}"
  if ! [[ "$want_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: --version wants a release triple such as 0.1.2" >&2
    exit 2
  fi
fi

uname_s="$(uname -s 2>/dev/null || true)"
case "$uname_s" in
  Darwin) platform=macos ;;
  Linux) platform=linux ;;
  MINGW*|MSYS*|CYGWIN*) platform=windows ;;
  *)
    if [[ "${OS:-}" == "Windows_NT" ]]; then
      platform=windows
    else
      echo "error: unsupported platform: ${uname_s:-unknown}" >&2
      exit 1
    fi
    ;;
esac

# `linux` is built on a current Ubuntu and needs glibc 2.32 or newer. An older
# loader cannot start those binaries at all, so those hosts take the `kylin`
# archive — the same pair built against glibc 2.31. A host that cannot report its
# glibc keeps the generic archive, as it did before the kylin channel existed.
host_glibc=""
glibc_line="$(getconf GNU_LIBC_VERSION 2>/dev/null || true)"
if [[ -z "$glibc_line" ]]; then
  glibc_line="$(ldd --version 2>/dev/null | head -1 || true)"
fi
candidate="${glibc_line##* }"
if [[ "$candidate" =~ ^[0-9]+\.[0-9]+$ ]]; then
  host_glibc="$candidate"
fi
if [[ "$platform" == "linux" && -n "$host_glibc" ]]; then
  glibc_major="${host_glibc%%.*}"
  glibc_minor="${host_glibc#*.}"
  if (( glibc_major < 2 )) || (( glibc_major == 2 && glibc_minor < 32 )); then
    platform=kylin
  fi
fi

case "$platform" in
  macos|linux|kylin) extension="tar.gz" ;;
  windows) extension="zip" ;;
esac

# The release workflow emits latest.json compact with this exact field order,
# so the shell can read it with strict patterns instead of a JSON tool.
manifest_json=""
local_asset=""
asset_url=""
asset_name=""
expected_sha=""
version=""
tag=""
release_base="${ASTER_RELEASE_BASE:-}"

if [[ -n "$direct_url" ]]; then
  asset_url="$direct_url"
  asset_name="${direct_url##*/}"
elif [[ -n "$release_base" ]]; then
  case "$release_base" in
    *x.ai*|*://*) refuse ;;
  esac
  if [[ ! -d "$release_base" ]]; then
    echo "error: ASTER_RELEASE_BASE is not a local release directory" >&2
    exit 1
  fi
  if [[ ! -f "${release_base}/latest.json" ]]; then
    echo "error: local release directory has no latest.json" >&2
    exit 1
  fi
  manifest_json="$(cat "${release_base}/latest.json")"
else
  if [[ -n "$want_version" ]]; then
    manifest_url="${official_releases}/download/aster-cli-v${want_version}/latest.json"
  else
    manifest_url="$latest_manifest"
  fi
  manifest_json="$(curl -fsSL --retry 3 --retry-delay 2 "$manifest_url")"
fi

if [[ -n "$manifest_json" ]]; then
  compact="$(printf '%s' "$manifest_json" | tr -d '[:space:]')"
  version="$(printf '%s' "$compact" | grep -oE '"version":"[0-9]+\.[0-9]+\.[0-9]+"' | head -1 | cut -d'"' -f4)"
  tag="$(printf '%s' "$compact" | grep -oE '"tag":"aster-cli-v[0-9]+\.[0-9]+\.[0-9]+"' | head -1 | cut -d'"' -f4)"
  if [[ -z "$version" || "$tag" != "aster-cli-v${version}" ]]; then
    echo "error: channel manifest is malformed" >&2
    exit 1
  fi
  if [[ -n "$want_version" && "$version" != "$want_version" ]]; then
    echo "error: release ${tag} does not match the requested version ${want_version}" >&2
    exit 1
  fi
  asset_name="aster-cli-v${version}-${platform}.${extension}"
  entry="$(printf '%s' "$compact" | grep -oE "\"${platform}\":\{\"name\":\"[^\"]+\",\"sha256\":\"[0-9a-f]{64}\"\}" | head -1)"
  expected_sha="$(printf '%s' "$entry" | grep -oE '[0-9a-f]{64}')"
  if [[ "$entry" != "\"${platform}\":{\"name\":\"${asset_name}\",\"sha256\":\"${expected_sha}\"}" ]]; then
    echo "error: channel manifest has no ${asset_name}" >&2
    exit 1
  fi
  if [[ -n "$release_base" ]]; then
    asset_url="${release_base}/${asset_name}"
    local_asset="$asset_url"
    if [[ ! -f "$asset_url" ]]; then
      echo "error: local release is missing ${asset_name}" >&2
      exit 1
    fi
  else
    asset_url="${official_releases}/download/${tag}/${asset_name}"
  fi
fi

if [[ -z "$prefix" ]]; then
  prefix="${HOME}/.local/bin"
fi

work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
archive="${work}/${asset_name}"
echo "platform=${platform}"
if [[ "$platform" == "kylin" ]]; then
  echo "glibc=${host_glibc}"
fi
echo "release=${tag:-direct}"
echo "asset=${asset_url}"

if [[ -n "$local_asset" ]]; then
  cp -f "$local_asset" "$archive"
else
  curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$asset_url"
fi

hash_of() {
  local result
  if command -v sha256sum >/dev/null 2>&1; then result="$(sha256sum -b "$1")";
  else result="$(shasum -a 256 -b "$1")"; fi
  printf '%s' "${result%% *}"
}

if [[ -n "$expected_sha" ]]; then
  if [[ "$(hash_of "$archive")" != "$expected_sha" ]]; then
    echo "error: channel checksum mismatch" >&2
    exit 1
  fi
fi

unpack="${work}/unpack"
mkdir -p "$unpack"
sandbox_files=(
  sandbox/a3s-oci
  sandbox/a3s-oci-krun-runtime/libkrun.1.17.0.dylib
  sandbox/a3s-oci-krun-runtime/libkrunfw.5.dylib
  sandbox/a3s-oci-krun-shim
  sandbox/a3s-oci-system.ext4
  sandbox/bundle/config.json
  sandbox/bundle/rootfs.tar
  sandbox/manifest.json
  sandbox/system-image.json
)
case "$asset_name" in
  *.tar.gz)
    tar -tzf "$archive" > "$work/members"
    seen="|"
    count=0
    while IFS= read -r name; do
      case "$name" in
        aster|a3s-code-acp|BUILD_ID|SHA256SUMS) ;;
        *)
          allowed=0
          if [[ "$platform" == macos ]]; then
            for member in "${sandbox_files[@]}"; do
              if [[ "$name" == "$member" ]]; then allowed=1; break; fi
            done
          fi
          [[ "$allowed" == 1 ]] || { echo "error: archive member is not a release file" >&2; exit 1; }
          ;;
      esac
      if [[ "$seen" == *"|${name}|"* ]]; then echo "error: duplicate archive member" >&2; exit 1; fi
      seen="${seen}${name}|"
      count=$((count + 1))
      [[ "$count" -le 13 ]] || { echo "error: too many archive members" >&2; exit 1; }
    done < "$work/members"
    tar -tvzf "$archive" > "$work/member-types"
    if LC_ALL=C grep -q '^[^-]' "$work/member-types"; then
      echo "error: release archive must contain regular files" >&2; exit 1
    fi
    tar -C "$unpack" -xzf "$archive"
    src_aster="${unpack}/aster"
    src_acp="${unpack}/a3s-code-acp"
    dest_aster="${prefix}/aster"
    dest_acp="${prefix}/a3s-code-acp"
    ;;
  *.zip)
    if command -v python3 >/dev/null 2>&1; then
      py=(python3)
    elif command -v python >/dev/null 2>&1; then
      py=(python)
    else
      echo "error: extracting the Windows archive needs python" >&2
      exit 1
    fi
    "${py[@]}" - "$archive" "$unpack" <<'PY'
import sys, zipfile
zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])
PY
    src_aster="${unpack}/aster.exe"
    src_acp="${unpack}/a3s-code-acp.exe"
    dest_aster="${prefix}/aster.exe"
    dest_acp="${prefix}/a3s-code-acp.exe"
    ;;
esac

if [[ ! -f "$src_aster" || ! -f "$src_acp" ]]; then
  echo "error: archive ${asset_name} did not contain both aster-cli binaries" >&2
  exit 1
fi

# Checksums cover the pair and every included sandbox companion.
[[ -f "$unpack/BUILD_ID" && -f "$unpack/SHA256SUMS" ]] || {
  echo "error: release is missing paired build identity or checksums" >&2; exit 1;
}
sha256_file() {
  local result
  if command -v sha256sum >/dev/null 2>&1; then result="$(sha256sum -b "$1")";
  else result="$(shasum -a 256 -b "$1")"; fi
  printf '%s  %s\n' "${result%% *}" "$1"
}
name_aster="${src_aster##*/}"
name_acp="${src_acp##*/}"
has_sandbox=0
if [[ -d "$unpack/sandbox" ]]; then has_sandbox=1; fi
if [[ "$platform" == macos && "$has_sandbox" != 1 ]]; then
  echo "error: macOS release is missing sandbox companions" >&2; exit 1
fi
expected_sums="$(
  cd "$unpack"
  sha256_file "$name_aster"
  sha256_file "$name_acp"
  if [[ "$has_sandbox" == 1 ]]; then
    for name in "${sandbox_files[@]}"; do
      [[ -f "$name" && ! -L "$name" ]] || { echo "error: release is missing sandbox companions" >&2; exit 1; }
      sha256_file "$name"
    done
  fi
)"
[[ "$(cat "$unpack/SHA256SUMS")" == "$expected_sums" ]] || {
  echo "error: paired binary checksum mismatch" >&2; exit 1;
}
chmod 755 "$src_aster" "$src_acp"
identity="$(cat "$unpack/BUILD_ID")"
[[ "$identity" == aster-cli-pair-v1:* && "$identity" == "$("$src_aster" --build-id)" && "$identity" == "$("$src_acp" --build-id)" ]] || {
  echo "error: paired binary source identity mismatch" >&2; exit 1;
}

mkdir -p "$prefix"
if [[ -d "$dest_aster" || -d "$dest_acp" ]]; then
  echo "error: binary destination is a directory" >&2; exit 1
fi
dest_sandbox="${prefix}/sandbox"
if [[ "$has_sandbox" == 1 && ( -L "$dest_sandbox" || ( -e "$dest_sandbox" && ! -d "$dest_sandbox" ) ) ]]; then
  echo "error: sandbox destination must be a regular directory" >&2; exit 1
fi
lock="${prefix}/.aster-install.lock"
mkdir "$lock" || { echo "error: another pair installation needs to finish" >&2; exit 1; }
stage="$(mktemp -d "${prefix}/.aster-install.XXXXXX")" || { rmdir "$lock"; exit 1; }
promoted_aster=0
promoted_acp=0
sandbox_backed_up=0
sandbox_promoted=0
installed=0
cleanup_pair() {
  local status=$?
  local recovered=1
  if [[ "$installed" == 0 ]]; then
    if [[ "$promoted_aster" == 1 ]]; then
      if [[ -e "$stage/old-aster" || -L "$stage/old-aster" ]]; then mv -f "$stage/old-aster" "$dest_aster" || recovered=0; else rm -f "$dest_aster" || recovered=0; fi
    fi
    if [[ "$promoted_acp" == 1 ]]; then
      if [[ -e "$stage/old-acp" || -L "$stage/old-acp" ]]; then mv -f "$stage/old-acp" "$dest_acp" || recovered=0; else rm -f "$dest_acp" || recovered=0; fi
    fi
    if [[ "$sandbox_promoted" == 1 ]]; then rm -rf "$dest_sandbox" || recovered=0; fi
    if [[ "$sandbox_backed_up" == 1 ]]; then
      if [[ ! -e "$dest_sandbox" && ! -L "$dest_sandbox" ]]; then
        mv "$stage/old-sandbox" "$dest_sandbox" || recovered=0
      else
        recovered=0
      fi
    fi
  fi
  if [[ "$recovered" == 0 ]]; then
    echo "error: pair recovery needs attention; backup retained at $stage" >&2
    rm -rf "$work"
    return 1
  fi
  rm -rf "$stage" "$work"
  rmdir "$lock"
  return "$status"
}
trap cleanup_pair EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cp -f "$src_aster" "$stage/new-aster"
cp -f "$src_acp" "$stage/new-acp"
chmod 755 "$stage/new-aster" "$stage/new-acp"
if [[ "$has_sandbox" == 1 ]]; then
  cp -R "$unpack/sandbox" "$stage/new-sandbox"
  chmod 755 "$stage/new-sandbox/a3s-oci" "$stage/new-sandbox/a3s-oci-krun-shim"
fi
[[ ! -e "$dest_aster" && ! -L "$dest_aster" ]] || cp -Pp "$dest_aster" "$stage/old-aster"
[[ ! -e "$dest_acp" && ! -L "$dest_acp" ]] || cp -Pp "$dest_acp" "$stage/old-acp"
mv -f "$stage/new-aster" "$dest_aster"
promoted_aster=1
# shortcut: two renames cannot survive every power loss; use one version-directory pointer when crash-atomic upgrades are required.
mv -f "$stage/new-acp" "$dest_acp"
promoted_acp=1
if [[ "$has_sandbox" == 1 ]]; then
  if [[ -d "$dest_sandbox" ]]; then
    mv "$dest_sandbox" "$stage/old-sandbox"
    sandbox_backed_up=1
  fi
  mv "$stage/new-sandbox" "$dest_sandbox"
  sandbox_promoted=1
fi
installed=1
if [[ "$platform" == "macos" ]] && command -v xattr >/dev/null 2>&1; then
  xattr -d com.apple.quarantine "$dest_aster" 2>/dev/null || true
  xattr -d com.apple.quarantine "$dest_acp" 2>/dev/null || true
fi

if [[ -n "$version" ]]; then
  echo "installed Aster v${version} into ${prefix}"
else
  echo "installed Aster into ${prefix}"
fi
echo "aster=${dest_aster}"
echo "a3s-code-acp=${dest_acp}"
