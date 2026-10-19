#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "Usage: $0 <new evidence directory> [view-commands|view-contract]" >&2
  exit 2
fi
fixture_mode="${2:-view-commands}"
case "$fixture_mode" in
  view-commands|view-contract) ;;
  *) echo "Unknown AIX view fixture" >&2; exit 2 ;;
esac
studio_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$studio_dir/../.." && pwd)"
case "$1" in
  /*) result_dir="$1" ;;
  *) result_dir="$PWD/$1" ;;
esac
mkdir "$result_dir"
export A3S_CLOUD_TEST_AIX_COMMANDS_PACKAGE="$result_dir/view-commands.aix"
export A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_OUTPUT="$result_dir/native-catalogue.json"
export A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_INPUT="$result_dir/native-catalogue.json"

cd "$studio_dir"
node scripts/build-aix-proof.mjs "$A3S_CLOUD_TEST_AIX_COMMANDS_PACKAGE" "$fixture_mode" > "$result_dir/package-build.log" 2>&1
cd "$repo_dir"
for lane in api catalogue; do
  if [[ "$lane" == api ]]; then
    manifest=apps/api/Cargo.toml
    target=(--bin a3s-api)
  else
    manifest=apps/studio/src-tauri/Cargo.toml
    target=(--lib)
  fi
  rustup run 1.97.1 cargo test --offline --manifest-path "$manifest" aix_view_commands_built "${target[@]}" -- --ignored --test-threads=1 > "$result_dir/$lane.log" 2>&1
  rg -q 'test result: ok\. 1 passed;' "$result_dir/$lane.log"
  echo "Passed: $lane"
done
if [[ "$fixture_mode" == view-contract ]]; then
  node --input-type=module - "$A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_OUTPUT" <<'NODE'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
const catalogue = JSON.parse(readFileSync(process.argv[2], 'utf8'))
assert.equal(catalogue.packages.length, 1)
assert.equal(catalogue.packages[0].views.length, 4)
for (const view of catalogue.packages[0].views) {
  assert.equal(view.contractVersion, 1)
  assert.equal(view.mode, 'append')
}
NODE
fi
cd "$studio_dir"
node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/plugins/__tests__/aixViewCommands.test.tsx > "$result_dir/renderer.log" 2>&1
rg -q 'Tests.*2 passed \(2\)' "$result_dir/renderer.log"
echo "AIX view command publication/native catalogue/executing document chain passed: $result_dir"
