#!/usr/bin/env bash
# Run on a Mac that can boot the packaged sandbox.
set -euo pipefail
: "${GH_REPO:?repository is required}"
: "${BUILD_SHA:?source commit is required}"
ARTIFACT_RUN_ID="${ARTIFACT_RUN_ID:-}"
shopt -s nullglob
files=(dist/*)
if [[ ${#files[@]} -ne 3 ]]; then
  echo "expected 3 release archives, found ${#files[@]}" >&2
  printf '%s\n' "${files[@]:-}" >&2
  exit 1
fi
if [[ -n "$ARTIFACT_RUN_ID" ]]; then
  built="$(gh api "repos/${GH_REPO}/actions/runs/${ARTIFACT_RUN_ID}" --jq .head_sha)"
else
  built="$BUILD_SHA"
fi
successful_runs="$(gh api "repos/${GH_REPO}/actions/workflows/qualify-aster-cli.yml/runs?head_sha=${built}&event=push&status=completed&per_page=100" --jq '[.workflow_runs[] | select(.conclusion == "success")] | length')"
if [[ "$successful_runs" -eq 0 ]]; then
  echo "refusing release: artifact source $built has no successful branch qualification" >&2
  exit 1
fi
python3 tests/cli_sandbox_smoke.py dist/aster-cli-v0.1.6-macos.tar.gz --output-dir qualification
python3 tests/cli_sandbox_smoke.py dist/aster-cli-v0.1.6-macos.tar.gz --mode yolo --output-dir qualification/yolo
macos_sha="$(shasum -a 256 -b dist/aster-cli-v0.1.6-macos.tar.gz | cut -d' ' -f1)"
linux_sha="$(shasum -a 256 -b dist/aster-cli-v0.1.6-linux.tar.gz | cut -d' ' -f1)"
windows_sha="$(shasum -a 256 -b dist/aster-cli-v0.1.6-windows.zip | cut -d' ' -f1)"
# Compact single-line manifest: scripts/install.sh parses this exact shape.
printf '{"version":"0.1.6","tag":"aster-cli-v0.1.6","assets":{"macos":{"name":"aster-cli-v0.1.6-macos.tar.gz","sha256":"%s"},"linux":{"name":"aster-cli-v0.1.6-linux.tar.gz","sha256":"%s"},"windows":{"name":"aster-cli-v0.1.6-windows.zip","sha256":"%s"}}}\n' \
  "$macos_sha" "$linux_sha" "$windows_sha" > dist/latest.json
cat > notes.md <<'EOF'
aster-cli v0.1.6 — Core / ACP 9.1.6

Image and image-only prompts now reach image-capable models. Explicit text-only models reject images before admission.
Model context limits follow configured or documented capacities and update when switching models. Unknown capacity remains unknown in the status bar.
Raw reasoning is folded by default and remains available on demand. TUI status chrome stays English independently of the model reply language; internal subagent lifecycle events no longer appear as reasoning text.
Subagent stop requests now reach their owning session and cancel the actual child. Failed stop requests remain retryable.

macOS archives include the verified sandbox runtime with Aster and ACP. Installation and self-update reject incomplete archives. The final installed macOS archive must execute a real sandbox command before release assets can be created or uploaded.
Each archive contains the locked release build of Aster 0.1.6 and ACP 9.1.6 from this repository, with their shared BUILD_ID and SHA256SUMS. The binaries are unsigned.
The Linux archive uses x86_64 glibc 2.31. Linux and Windows runtime support is not qualified by the macOS sandbox probe.
Update with `aster update`, or inspect with `aster update --check`, then restart the TUI to use the new pair.
EOF
printf 'Built from %s.\n' "$built" >> notes.md
# A rerun may extend this draft, never replace published assets.
if gh release view aster-cli-v0.1.6 >/dev/null 2>&1; then
  test "$(gh release view aster-cli-v0.1.6 --json isDraft --jq .isDraft)" = true
  test "$(gh release view aster-cli-v0.1.6 --json targetCommitish --jq .targetCommitish)" = "$built"
  gh release upload aster-cli-v0.1.6 "${files[@]}" dist/latest.json --clobber
else
  gh release create aster-cli-v0.1.6 \
    --draft --target "$built" \
    --latest=false \
    --title "aster-cli v0.1.6" \
    --notes-file notes.md \
    "${files[@]}" dist/latest.json
fi
gh release view aster-cli-v0.1.6
