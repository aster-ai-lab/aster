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
export A3S_CLOUD_TEST_AGENT_FORM_OUTPUT="$result_dir/native-form.json"
export A3S_CLOUD_TEST_AGENT_FORM_INPUT="$A3S_CLOUD_TEST_AGENT_FORM_OUTPUT"
cd "$repo_dir"
rustup run 1.97.1 cargo test --offline --manifest-path apps/studio/src-tauri/Cargo.toml --lib native_owned_form_contract_built_for_sdk_and_renderer -- --ignored --test-threads=1 > "$result_dir/native.log" 2>&1
rg -q 'test result: ok\. 1 passed;' "$result_dir/native.log"
cd "$studio_dir"
node node_modules/vitest/vitest.mjs run --project renderer src/renderer/components/chat/form/__tests__/nativeAgentForm.test.tsx > "$result_dir/renderer.log" 2>&1
rg -q 'Tests.*1 passed \(1\)' "$result_dir/renderer.log"
echo "Native owned form / SDK / renderer contract passed: $result_dir"
