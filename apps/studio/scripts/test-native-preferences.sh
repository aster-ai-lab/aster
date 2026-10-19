#!/usr/bin/env bash
set -euo pipefail

studio_root="$(cd "$(dirname "$0")/.." && pwd)"
output="${1:?usage: test-native-preferences.sh <new evidence directory>}"
if [[ -e "$output" ]]; then
  printf '%s\n' 'Evidence directory already exists; choose a new directory.' >&2
  exit 1
fi
mkdir -p "$output"
output="$(cd "$output" && pwd)"
cd "$studio_root"

node scripts/data-classify/scripts/generate-preferences.js --check-native > "$output/freshness.log" 2>&1
rustup run 1.97.1 cargo test --offline --manifest-path src-tauri/Cargo.toml --lib preferences::tests \
  -- --test-threads=1 > "$output/native.log" 2>&1
rustup run 1.97.1 cargo test --offline --manifest-path src-tauri/Cargo.toml --lib --no-run \
  --message-format=json > "$output/native-build.jsonl" 2> "$output/native-build.log"
preference_executable="$(python3 - "$output/native-build.jsonl" <<'PY'
import json, sys
executables = []
for line in open(sys.argv[1]):
    item = json.loads(line)
    if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == 'a3s_studio_lib' and item.get('executable'):
        executables.append(item['executable'])
if len(executables) != 1:
    raise SystemExit('one native Preference proof executable required')
print(executables[0])
PY
)"
A3S_CLOUD_TEST_PREFERENCE_EXECUTABLE="$preference_executable" \
  node node_modules/vitest/vitest.mjs run --project renderer \
  src/renderer/shell/studioNativePreference.test.ts src/renderer/shell/studioHarnessPreference.test.ts \
  src/renderer/services/__tests__/harnessAssemble.test.ts > "$output/bridge.log" 2>&1
