#!/usr/bin/env bash
# Installs aster-cli from a local stand-in release directory with a channel
# manifest. This check does not start a release build. Pass an evidence
# directory to keep logs.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
install="${repo_root}/scripts/install.sh"
ps1="${repo_root}/scripts/install.ps1"
package="${repo_root}/scripts/package-aster-cli.sh"
workflow="${repo_root}/.github/workflows/release-aster-cli.yml"
readme="${repo_root}/README.md"
evidence="${1:-${ASTER_INSTALL_EVIDENCE:-}}"
manifest_url="https://github.com/aster-ai-lab/aster/releases/download/aster-cli-stable/latest.json"

pkg_version="$(
  awk -F'"' '/^version = / { print $2; exit }' "${repo_root}/apps/aster/Cargo.toml"
)"
acp_version="$(
  awk -F'"' '/^version = / { print $2; exit }' "${repo_root}/crates/code/acp/Cargo.toml"
)"
if [[ "$pkg_version" != "0.1.2" ]]; then
  echo "error: apps/aster version is ${pkg_version}, want 0.1.2" >&2
  exit 1
fi
if [[ -z "$acp_version" ]]; then
  echo "error: a3s-code-acp package version is empty" >&2
  exit 1
fi

for path in "$install" "$ps1" "$package" "$workflow" "$readme"; do
  [[ -f "$path" ]]
done
if grep -E 'https://x\.ai/cli|git clone|cargo build' "$install" "$ps1"; then
  echo "error: install entry clones, compiles, or fetches an upstream CLI" >&2
  exit 1
fi
# Both installers discover the newest release through the channel manifest.
grep -q "$manifest_url" "$install"
grep -q "$manifest_url" "$ps1"
grep -q 'macos-latest' "$workflow"
grep -q 'ubuntu-latest' "$workflow"
grep -q 'windows-latest' "$workflow"
grep -q 'contents: write' "$workflow"
grep -q 'cargo build --locked --release -p aster -p a3s-code-acp' "$workflow"
grep -q "aster-cli-v${pkg_version}-macos.tar.gz" "$workflow"
grep -q "aster-cli-v${pkg_version}-linux.tar.gz" "$workflow"
grep -q "aster-cli-v${pkg_version}-windows.zip" "$workflow"
# The publish job regenerates the manifest with fresh digests.
grep -q 'dist/latest.json' "$workflow"
grep -q 'sha256sum -b' "$workflow"

section="$(awk '/^## 开始使用$/{flag=1} flag{print} /^## 运行机制$/{flag=0}' "$readme")"
if grep -E 'git clone|cargo build' <<<"$section"; then
  echo "error: README install section clones or compiles" >&2
  exit 1
fi
grep -q 'releases/latest' <<<"$section"
grep -q 'scripts/install.sh' <<<"$section"
grep -q 'scripts/install.ps1' <<<"$section"

work="$(mktemp -d "${TMPDIR:-/var/folders}/aster-install-check.XXXXXX")"
save_evidence() {
  [[ -n "$evidence" ]] || return 0
  mkdir -p "$evidence"
  local f
  shopt -s nullglob
  for f in "$work"/*.out "$work"/*.err "$work"/*.json "$work"/*.txt; do
    cp -f "$f" "$evidence/"
  done
}
cleanup() {
  local code=$?
  save_evidence
  rm -rf "$work"
  exit "$code"
}
trap cleanup EXIT

mkdir -p "$work/bin" "$work/dist" "$work/macos-bin" "$work/linux-bin" "$work/windows-bin"
cat >"$work/bin/git" <<'EOF'
#!/bin/sh
printf '%s\n' "git $*" >> "${TOOL_LOG:?}"
exit 99
EOF
cat >"$work/bin/cargo" <<'EOF'
#!/bin/sh
printf '%s\n' "cargo $*" >> "${TOOL_LOG:?}"
exit 99
EOF
cat >"$work/bin/curl" <<'EOF'
#!/bin/sh
printf '%s\n' "curl $*" >> "${TOOL_LOG:?}"
exit 99
EOF
chmod 755 "$work/bin/git" "$work/bin/cargo" "$work/bin/curl"
: >"$work/tool.log"

cat >"$work/macos-bin/aster" <<EOF
#!/bin/sh
if [[ "\$1" == "--build-id" ]]; then
  printf '%s\n' 'aster-cli-pair-v1:install-check:local'
  exit 0
fi
if [[ "\$1" == "version" ]]; then
  printf '%s\n' '{"product":"Aster","currentVersion":"${pkg_version} (stand-in)","channel":"unknown"}'
  exit 0
fi
if [[ "\$1" == "update" ]]; then
  printf '%s\n' '{"currentVersion":"${pkg_version} (stand-in)","latestVersion":null,"updateAvailable":false,"installer":null,"channel":"unknown","autoUpdate":false,"error":"stand-in"}'
  exit 0
fi
exit 2
EOF
cat >"$work/macos-bin/a3s-code-acp" <<EOF
#!/bin/sh
if [[ "\$1" == "--build-id" ]]; then
  printf '%s\n' 'aster-cli-pair-v1:install-check:local'
  exit 0
fi
printf '%s\n' '${acp_version}'
exit 0
EOF
chmod 755 "$work/macos-bin/aster" "$work/macos-bin/a3s-code-acp"
pair_stub() {
  printf '%s\n' '#!/bin/sh' 'if [ "$1" = "--build-id" ]; then' "  printf '%s\\n' 'aster-cli-pair-v1:install-check:local'" '  exit 0' 'fi' 'exit 0' >"$1"
  chmod 755 "$1"
}
pair_stub "$work/linux-bin/aster"
pair_stub "$work/linux-bin/a3s-code-acp"
pair_stub "$work/windows-bin/aster.exe"
pair_stub "$work/windows-bin/a3s-code-acp.exe"

"$package" macos "$work/macos-bin" "$work/dist/aster-cli-v${pkg_version}-macos.tar.gz"
"$package" linux "$work/linux-bin" "$work/dist/aster-cli-v${pkg_version}-linux.tar.gz"
"$package" windows "$work/windows-bin" "$work/dist/aster-cli-v${pkg_version}-windows.zip"
tar -tzf "$work/dist/aster-cli-v${pkg_version}-macos.tar.gz" | sort >"$work/macos-members.txt"
tar -tzf "$work/dist/aster-cli-v${pkg_version}-linux.tar.gz" | sort >"$work/linux-members.txt"
python3 - "$work/dist/aster-cli-v${pkg_version}-windows.zip" >"$work/windows-members.txt" <<'PY'
import sys, zipfile
print("\n".join(sorted(zipfile.ZipFile(sys.argv[1]).namelist())))
PY
grep -qx 'a3s-code-acp' "$work/macos-members.txt"
grep -qx 'aster' "$work/macos-members.txt"
grep -qx 'a3s-code-acp' "$work/linux-members.txt"
grep -qx 'aster' "$work/linux-members.txt"
grep -qx 'a3s-code-acp.exe' "$work/windows-members.txt"
grep -qx 'aster.exe' "$work/windows-members.txt"

hash_of() {
  local result
  if command -v sha256sum >/dev/null 2>&1; then result="$(sha256sum -b "$1")";
  else result="$(shasum -a 256 -b "$1")"; fi
  printf '%s' "${result%% *}"
}

# The compact channel manifest, in the exact shape the workflow publishes.
macos_sha="$(hash_of "$work/dist/aster-cli-v${pkg_version}-macos.tar.gz")"
linux_sha="$(hash_of "$work/dist/aster-cli-v${pkg_version}-linux.tar.gz")"
windows_sha="$(hash_of "$work/dist/aster-cli-v${pkg_version}-windows.zip")"
printf '{"version":"%s","tag":"aster-cli-v%s","assets":{"macos":{"name":"aster-cli-v%s-macos.tar.gz","sha256":"%s"},"linux":{"name":"aster-cli-v%s-linux.tar.gz","sha256":"%s"},"windows":{"name":"aster-cli-v%s-windows.zip","sha256":"%s"}}}\n' \
  "$pkg_version" "$pkg_version" "$pkg_version" "$macos_sha" "$pkg_version" "$linux_sha" "$pkg_version" "$windows_sha" \
  >"$work/dist/latest.json"

run_install() {
  local prefix="$1"
  local stem="$2"
  TOOL_LOG="$work/tool.log" PATH="$work/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    ASTER_RELEASE_BASE="$work/dist" \
    bash "$install" --prefix "$prefix" >"${stem}.out" 2>"${stem}.err"
}

refuse_prefix="${work}/refuse-prefix"
set +e
TOOL_LOG="$work/tool.log" PATH="$work/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
  bash "$install" --prefix "$refuse_prefix" "https://x.ai/cli" \
  >"$work/refuse.out" 2>"$work/refuse.err"
refuse_code=$?
set -e
if [[ "$refuse_code" -eq 0 ]]; then
  echo "error: upstream CLI URL was accepted" >&2
  exit 1
fi
if [[ -e "$refuse_prefix" ]]; then
  echo "error: refused install created a prefix" >&2
  exit 1
fi
if [[ -s "$work/tool.log" ]]; then
  echo "error: refused install downloaded or built" >&2
  cat "$work/tool.log" >&2
  exit 1
fi
grep -q 'does not download that binary' "$work/refuse.err"

set +e
TOOL_LOG="$work/tool.log" PATH="$work/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
  ASTER_RELEASE_BASE="https://x.ai/cli" \
  bash "$install" --prefix "$refuse_prefix" \
  >"$work/refuse-base.out" 2>"$work/refuse-base.err"
base_code=$?
set -e
if [[ "$base_code" -eq 0 || -e "$refuse_prefix" || -s "$work/tool.log" ]]; then
  echo "error: upstream release base was accepted" >&2
  exit 1
fi

prefix_a="${work}/prefix-a"
run_install "$prefix_a" "$work/prefix-a-run1"
if [[ -s "$work/tool.log" ]]; then
  echo "error: install invoked git, cargo, or curl" >&2
  cat "$work/tool.log" >&2
  exit 1
fi
[[ -x "${prefix_a}/aster" ]]
[[ -x "${prefix_a}/a3s-code-acp" ]]
grep -q "release=aster-cli-v${pkg_version}" "$work/prefix-a-run1.out"
grep -q "asset=${work}/dist/aster-cli-v${pkg_version}-macos.tar.gz" "$work/prefix-a-run1.out"
if grep -E 'Cloning|Compiling' "$work/prefix-a-run1.out" "$work/prefix-a-run1.err"; then
  echo "error: install cloned or compiled" >&2
  exit 1
fi
"${prefix_a}/aster" version --json >"$work/prefix-a-run1-version.json"
"${prefix_a}/a3s-code-acp" --version >"$work/prefix-a-acp-version.txt"
got_acp="$(tr -d '[:space:]' <"$work/prefix-a-acp-version.txt")"
if [[ "$got_acp" != "$acp_version" ]]; then
  echo "error: acp version ${got_acp} != package ${acp_version}" >&2
  exit 1
fi
python3 - "$pkg_version" "$work/prefix-a-run1-version.json" <<'PY'
import json, sys
want, path = sys.argv[1], sys.argv[2]
body = json.loads(open(path, encoding="utf-8").read())
version = str(body.get("currentVersion", ""))
if body.get("product") != "Aster":
    raise SystemExit(f"product {body.get('product')!r}")
if not version.startswith(want):
    raise SystemExit(f"currentVersion {version!r} does not start with {want}")
PY
before="$(stat -f '%z' "${prefix_a}/aster")"
echo "$before" >"$work/size-before.txt"
"${prefix_a}/aster" update --check --json >"$work/prefix-a-update.json"
after="$(stat -f '%z' "${prefix_a}/aster")"
echo "$after" >"$work/size-after.txt"
if [[ "$before" != "$after" ]]; then
  echo "error: update --check changed aster from ${before} to ${after}" >&2
  exit 1
fi
python3 - "$work/prefix-a-update.json" <<'PY'
import json, sys
body = json.loads(open(sys.argv[1], encoding="utf-8").read())
if body.get("updateAvailable") is not False:
    raise SystemExit(f"updateAvailable {body.get('updateAvailable')!r}")
if body.get("autoUpdate") is not False:
    raise SystemExit(f"autoUpdate {body.get('autoUpdate')!r}")
PY

# A corrupted archive behind a valid manifest must refuse without touching
# the installed pair, then the restored archive reinstalls cleanly.
macos_archive="$work/dist/aster-cli-v${pkg_version}-macos.tar.gz"
cp -f "$macos_archive" "$work/dist/.macos-archive-good"
printf 'tampered' >>"$macos_archive"
set +e
TOOL_LOG="$work/tool.log" PATH="$work/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
  ASTER_RELEASE_BASE="$work/dist" \
  bash "$install" --prefix "$prefix_a" >"$work/digest-fault.out" 2>"$work/digest-fault.err"
digest_code=$?
set -e
if [[ "$digest_code" -eq 0 ]]; then
  echo "error: corrupt archive was accepted" >&2
  exit 1
fi
grep -q 'channel checksum mismatch' "$work/digest-fault.err"
mv -f "$work/dist/.macos-archive-good" "$macos_archive"

run_install "$prefix_a" "$work/prefix-a-run2"
"${prefix_a}/aster" version --json >"$work/prefix-a-run2-version.json"
if ! cmp -s "$work/prefix-a-run1-version.json" "$work/prefix-a-run2-version.json"; then
  echo "error: second install changed version text" >&2
  exit 1
fi
if [[ -s "$work/tool.log" ]]; then
  echo "error: second install invoked git, cargo, or curl" >&2
  cat "$work/tool.log" >&2
  exit 1
fi

cat >"$work/platforms.txt" <<EOF
executed=macos
archive_checked=macos,linux,windows
host_os=$(uname -s)
entry=${install}
windows_branch=${ps1}
package_version=${pkg_version}
acp_package_version=${acp_version}
channel=${manifest_url}
EOF

echo "INSTALL_CHECK_OK version=${pkg_version}"
