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

run_case() {
  local report="$1"
  shift
  if ! "$@" > "$result_dir/$report" 2>&1; then
    echo "Failed: $report; retained output: $result_dir/$report" >&2
    return 1
  fi
  if [[ "$report" != renderer.log ]] && ! rg -q 'test result: ok\. [1-9][0-9]* passed;' "$result_dir/$report"; then
    echo "No passing Rust cases: $result_dir/$report" >&2
    return 1
  fi
  echo "Passed: $report"
}

export A3S_CLOUD_TEST_AIX_EXECUTION_OBSERVATIONS="$result_dir/acp-observations.jsonl"
export A3S_CLOUD_TEST_AIX_EXECUTION_SSE_OUTPUT="$result_dir/native-sse.jsonl"
export A3S_CLOUD_TEST_AIX_EXECUTION_SSE_INPUT="$result_dir/native-sse.jsonl"

cd "$repo_dir"
run_case core.log rustup run 1.97.1 cargo test --offline --manifest-path apps/code/Cargo.toml \
  -p a3s-code-core agent_api::tests::stream_identity:: --lib -- --test-threads=1
run_case acp.log rustup run 1.97.1 cargo test --offline --manifest-path apps/code/Cargo.toml \
  -p a3s-code-acp run_observation --lib -- --test-threads=1
run_case native.log rustup run 1.97.1 cargo test --offline --manifest-path apps/studio/src-tauri/Cargo.toml \
  retained_core_runs_cross_the_native_observer_and_real_sse_encoder --lib -- --ignored --test-threads=1

cd "$studio_dir"
run_case renderer.log node node_modules/vitest/vitest.mjs run --project renderer \
  src/renderer/data/__tests__/studioAgentPlane.test.ts \
  src/renderer/components/plugins/__tests__/aixComposer.test.tsx
echo "Core/ACP/native SSE/renderer observation transport passed: $result_dir"
