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

run_rust() {
  local report="$1"
  shift
  if ! "$@" > "$result_dir/$report" 2>&1; then
    echo "Failed: $report; retained output: $result_dir/$report" >&2
    return 1
  fi
  if ! rg -q 'test result: ok\. [1-9][0-9]* passed;' "$result_dir/$report"; then
    echo "No passing Rust cases: $result_dir/$report" >&2
    return 1
  fi
  echo "Passed: $report"
}

export A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_PACKAGE="$result_dir/composer-execution.aix"
export A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_CATALOGUE_OUTPUT="$result_dir/native-catalogue.json"
export A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_CATALOGUE_INPUT="$result_dir/native-catalogue.json"

cd "$studio_dir"
node scripts/build-aix-proof.mjs "$A3S_CLOUD_TEST_AIX_EXECUTION_COMPOSER_PACKAGE" composer-execution > "$result_dir/package-build.log" 2>&1

cd "$repo_dir"
run_rust api.log rustup run 1.97.1 cargo test --offline --manifest-path apps/api/Cargo.toml aix_execution_composer_built --bin a3s-api -- --ignored --test-threads=1
run_rust catalogue.log rustup run 1.97.1 cargo test --offline --manifest-path apps/studio/src-tauri/Cargo.toml aix_execution_composer_built --lib -- --ignored --test-threads=1

bash "$studio_dir/scripts/test-aix-execution-observations.sh" "$result_dir/execution" > "$result_dir/observation-chain.log" 2>&1
echo "AIX API admission/native installation/Core SSE/renderer provider chain passed: $result_dir"
