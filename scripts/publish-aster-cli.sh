#!/usr/bin/env bash
# Run on a Mac that can verify the aster + aster-acp pair archives.
set -euo pipefail
: "${GH_REPO:?repository is required}"
: "${BUILD_SHA:?source commit is required}"
ARTIFACT_RUN_ID="${ARTIFACT_RUN_ID:-}"
shopt -s nullglob
files=(dist/*)
# The Kylin x86_64 named channel is optional: republishing an older four-archive
# qualification run must still work. A fifth file is only allowed if it is the
# byte-identical Kylin alias of the linux archive (checked below).
if [[ ${#files[@]} -ne 4 && ${#files[@]} -ne 5 ]]; then
  echo "expected 4 or 5 release archives, found ${#files[@]}" >&2
  printf '%s\n' "${files[@]:-}" >&2
  exit 1
fi
if [[ -n "$ARTIFACT_RUN_ID" ]]; then
  built="$(gh api "repos/${GH_REPO}/actions/runs/${ARTIFACT_RUN_ID}" --jq .head_sha)"
else
  built="$BUILD_SHA"
fi
source_commit="$(git rev-parse HEAD)"
source_status="$(git status --porcelain -- . ':!dist' ':!qualification' ':!notes.md')"
if [[ "$source_commit" != "$built" || -n "$source_status" ]]; then
  echo "refusing release: publish from the clean artifact source commit $built" >&2
  exit 1
fi
version="$(cargo read-manifest --manifest-path apps/aster/Cargo.toml | python3 -c 'import json, sys; print(json.load(sys.stdin)["version"])')"
acp_version="$(cargo read-manifest --manifest-path crates/code/acp/Cargo.toml | python3 -c 'import json, sys; print(json.load(sys.stdin)["version"])')"
core_version="$(cargo read-manifest --manifest-path crates/code/core/Cargo.toml | python3 -c 'import json, sys; print(json.load(sys.stdin)["version"])')"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ || "$acp_version" != "$core_version" ]]; then
  echo "refusing release: invalid CLI version or mismatched Core/ACP versions" >&2
  exit 1
fi
tag="aster-cli-v${version}"
for asset in "${tag}-macos.tar.gz" "${tag}-linux.tar.gz" "${tag}-linux-arm64.tar.gz" "${tag}-windows.zip"; do
  if [[ ! -f "dist/$asset" ]]; then
    echo "missing CLI release archive: $asset" >&2
    exit 1
  fi
done
# Reject stray or retired names instead of uploading them as release assets.
for file in "${files[@]}"; do
  case "${file##*/}" in
    "${tag}-macos.tar.gz"|"${tag}-linux.tar.gz"|"${tag}-linux-arm64.tar.gz"|\
    "${tag}-kylin-x86_64.tar.gz"|"${tag}-windows.zip") ;;
    *) echo "unexpected release archive: ${file##*/}" >&2; exit 1 ;;
  esac
done
kylin_asset="dist/${tag}-kylin-x86_64.tar.gz"
if [[ -f "$kylin_asset" && ${#files[@]} -ne 5 ]]; then
  echo "expected 5 release archives with the Kylin alias, found ${#files[@]}" >&2
  exit 1
fi
successful_runs="$(gh api "repos/${GH_REPO}/actions/workflows/qualify-aster-cli.yml/runs?head_sha=${built}&event=push&status=completed&per_page=100" --jq '[.workflow_runs[] | select(.conclusion == "success")] | length')"
if [[ "$successful_runs" -eq 0 ]]; then
  echo "refusing release: artifact source $built has no successful branch qualification" >&2
  exit 1
fi
python3 - "$tag" <<'PY'
from pathlib import Path
import sys
import tarfile

tag = sys.argv[1]
archive = Path("dist") / f"{tag}-macos.tar.gz"
with tarfile.open(archive, "r:gz") as tf:
    names = set(tf.getnames())
expected = {
    "aster",
    "aster-acp",
    "BUILD_ID",
    "SHA256SUMS",
    "embedding/potion-code-16M-v2.safetensors",
    "embedding/potion-code-16M-v2.tokenizer.json",
    "embedding/potion-code-16M-v2.config.json",
}
if names != expected:
    raise SystemExit(f"unexpected archive members: {sorted(names)}")
print(f"verified archive members for {archive.name}")
PY
macos_sha="$(shasum -a 256 -b "dist/${tag}-macos.tar.gz" | cut -d' ' -f1)"
linux_sha="$(shasum -a 256 -b "dist/${tag}-linux.tar.gz" | cut -d' ' -f1)"
linux_arm64_sha="$(shasum -a 256 -b "dist/${tag}-linux-arm64.tar.gz" | cut -d' ' -f1)"
windows_sha="$(shasum -a 256 -b "dist/${tag}-windows.zip" | cut -d' ' -f1)"
# The optional Kylin channel is a name, not a second build: publishing a
# different archive under that name would silently fork the x86_64 baseline.
kylin_entry=""
if [[ -f "$kylin_asset" ]]; then
  kylin_sha="$(shasum -a 256 -b "$kylin_asset" | cut -d' ' -f1)"
  if [[ "$kylin_sha" != "$linux_sha" ]]; then
    echo "error: Kylin x86_64 alias must be byte-identical to the linux archive" >&2
    exit 1
  fi
  kylin_entry=',"kylin-x86_64":{"name":"'"${tag}"'-kylin-x86_64.tar.gz","sha256":"'"${kylin_sha}"'"}'
fi
# Compact single-line manifest: scripts/install.sh parses this exact shape.
printf '{"version":"%s","tag":"%s","assets":{"macos":{"name":"%s-macos.tar.gz","sha256":"%s"},"linux":{"name":"%s-linux.tar.gz","sha256":"%s"},"linux-arm64":{"name":"%s-linux-arm64.tar.gz","sha256":"%s"},"windows":{"name":"%s-windows.zip","sha256":"%s"}%s}}\n' \
  "$version" "$tag" "$tag" "$macos_sha" "$tag" "$linux_sha" "$tag" "$linux_arm64_sha" "$tag" "$windows_sha" "$kylin_entry" > dist/latest.json
cat > notes.md <<EOF
aster-cli v${version} — Core / ACP ${acp_version}

Aster CLI ships the aster + aster-acp pair plus the offline Model2Vec embedding assets under embedding/. Process isolation uses the locked ksandbox dependency in Core, not companions or kbox MicroVM inside this archive.
Each archive contains the locked release build of Aster ${version} and ACP ${acp_version} from this repository, with their shared BUILD_ID and SHA256SUMS. The binaries are unsigned.
Linux archives use glibc 2.31: \`linux\` is x86_64 and \`linux-arm64\` is aarch64.
When published, \`kylin-x86_64\` is a byte-identical named channel of \`linux\` for Kylin V10 SP1 x86_64; \`aster update\` keeps using the \`linux\` channel.
Update with \`aster update\`, or inspect with \`aster update --check\`, then restart the TUI to use the new pair.
EOF
printf 'Built from %s.\n' "$built" >> notes.md
# A rerun may extend this draft, never replace published assets.
if gh release view "$tag" >/dev/null 2>&1; then
  test "$(gh release view "$tag" --json isDraft --jq .isDraft)" = true
  test "$(gh release view "$tag" --json targetCommitish --jq .targetCommitish)" = "$built"
  gh release upload "$tag" "${files[@]}" dist/latest.json --clobber
else
  gh release create "$tag" \
    --draft --target "$built" \
    --latest=false \
    --title "aster-cli v${version}" \
    --notes-file notes.md \
    "${files[@]}" dist/latest.json
fi
gh release view "$tag"
