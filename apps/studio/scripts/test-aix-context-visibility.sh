#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <new evidence directory>" >&2
  exit 2
fi
studio_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$studio_dir/../.." && pwd)"
case "$1" in
  /*) result_dir="$1" ;;
  *) result_dir="$PWD/$1" ;;
esac
mkdir "$result_dir"
export A3S_CLOUD_TEST_AIX_CONTEXT_PACKAGE="$result_dir/context-visibility.aix"
export A3S_CLOUD_TEST_AIX_CONTEXT_CATALOGUE_OUTPUT="$result_dir/native-catalogue.json"
export A3S_CLOUD_TEST_AIX_CONTEXT_CATALOGUE_INPUT="$result_dir/native-catalogue.json"

cd "$studio_dir"
node scripts/build-aix-proof.mjs "$A3S_CLOUD_TEST_AIX_CONTEXT_PACKAGE" context-visibility > "$result_dir/package-build.log" 2>&1
cd "$repo_dir"
for lane in api catalogue; do
  if [[ "$lane" == api ]]; then
    manifest=apps/api/Cargo.toml
    target=(--bin a3s-api)
  else
    manifest=apps/studio/src-tauri/Cargo.toml
    target=(--lib)
  fi
  rustup run 1.97.1 cargo test --offline --manifest-path "$manifest" aix_context_visibility_built "${target[@]}" -- --ignored --test-threads=1 > "$result_dir/$lane.log" 2>&1
  rg -q 'test result: ok\. 1 passed;' "$result_dir/$lane.log"
  echo "Passed: $lane"
done
cd "$studio_dir"
node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/plugins/__tests__/aixWorkbench.test.tsx -t 'conditional mini app|container and per-view conditions' > "$result_dir/renderer.log" 2>&1
rg -q 'Tests.*2 passed' "$result_dir/renderer.log"
echo "AIX context publication/native catalogue/executable renderer chain passed: $result_dir"
