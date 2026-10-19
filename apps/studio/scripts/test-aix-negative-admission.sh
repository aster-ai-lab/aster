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
cd "$studio_dir"
cd "$repo_dir"
for case_mode in unknown-field unsupported-manifest-schema unsupported-contract-version unsupported-port-version missing-dependency duplicate-contribution dependency-cycle path-traversal digest-substitution cross-package-target missing-service; do
  export A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE="$result_dir/$case_mode.aix"
  export A3S_CLOUD_TEST_AIX_NEGATIVE_CASE="$case_mode"
  export A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT="$result_dir/$case_mode-native-catalogue.json"
  export A3S_STUDIO_TEST_AIX_NEGATIVE_CATALOGUE="$A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT"
  export A3S_STUDIO_TEST_AIX_MISSING_SERVICE_CATALOGUE="$A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT"
  export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=unsupported
  if [[ "$case_mode" == duplicate-contribution ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=unique
  elif [[ "$case_mode" == unsupported-manifest-schema ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION="only package manifest schema version 3 is supported"
  elif [[ "$case_mode" == dependency-cycle ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=itself
  elif [[ "$case_mode" == path-traversal ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=escapes
  elif [[ "$case_mode" == digest-substitution ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=digest
  elif [[ "$case_mode" == cross-package-target ]]; then
    export A3S_CLOUD_TEST_AIX_NEGATIVE_EXPECTATION=owned
  fi
  cd "$studio_dir"
  node scripts/build-aix-proof.mjs "$A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE" "$case_mode" \
    > "$result_dir/$case_mode-package-build.log" 2>&1
  cd "$repo_dir"
  api_test=aix_invalid_built_artifact_is_rejected_before_publication
  native_test=aix_invalid_built_artifact_is_rejected_without_catalogue_contributions
  renderer_pattern='does not admit contributions from the native catalogue'
  renderer_file=src/renderer/components/plugins/__tests__/aixComposer.test.tsx
  if [[ "$case_mode" == digest-substitution || "$case_mode" == missing-service || "$case_mode" == unsupported-port-version || "$case_mode" == missing-dependency ]]; then
    api_test=aix_valid_built_artifact_is_staged_from_exact_bytes
  fi
  if [[ "$case_mode" == missing-service ]]; then
    native_test=aix_missing_service_built_artifact_projects_workspace_view
    renderer_pattern='blocks AIX workspace contributions when the required host service is unavailable'
  fi
  if [[ "$case_mode" == unsupported-port-version ]]; then
    native_test=aix_view_commands_built_artifact_projects_commands_and_assets
    renderer_file=src/renderer/components/plugins/__tests__/aixViewCommands.test.tsx
    renderer_pattern='withdraws an installed guest that answers its command handshake with an unsupported port version'
    export A3S_CLOUD_TEST_AIX_COMMANDS_PACKAGE="$A3S_CLOUD_TEST_AIX_NEGATIVE_PACKAGE"
    export A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_OUTPUT="$A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT"
    export A3S_CLOUD_TEST_AIX_COMMANDS_CATALOGUE_INPUT="$A3S_CLOUD_TEST_AIX_NEGATIVE_CATALOGUE_OUTPUT"
  fi
  if [[ "$case_mode" == missing-dependency ]]; then
    native_test=aix_selection_rejects_built_artifact_without_exact_dependency_pin
  fi
  rustup run 1.97.1 cargo test --offline --manifest-path apps/api/Cargo.toml --bin a3s-api \
    "$api_test" -- --ignored --test-threads=1 > "$result_dir/$case_mode-api.log" 2>&1
  rg -q 'test result: ok\. 1 passed;' "$result_dir/$case_mode-api.log"
  rustup run 1.97.1 cargo test --offline --manifest-path apps/studio/src-tauri/Cargo.toml --lib \
    "$native_test" -- --ignored --test-threads=1 \
    > "$result_dir/$case_mode-native.log" 2>&1
  rg -q 'test result: ok\. 1 passed;' "$result_dir/$case_mode-native.log"
  cd "$studio_dir"
  node node_modules/vitest/vitest.mjs run --project renderer \
    "$renderer_file" \
    -t "$renderer_pattern" > "$result_dir/$case_mode-renderer.log" 2>&1
  rg -q 'Tests.*1 passed' "$result_dir/$case_mode-renderer.log"
done
shasum -a 256 "$result_dir"/* \
  "$studio_dir/scripts/build-aix-proof.mjs" "$studio_dir/scripts/test-aix-negative-admission.sh" \
  "$repo_dir/apps/api/src/contexts/plugin/application/aix_tests.rs" \
  "$studio_dir/src-tauri/src/capabilities_aix_tests.rs" \
  "$studio_dir/src-tauri/src/capabilities_selection_tests.rs" \
  "$studio_dir/src/renderer/components/plugins/__tests__/aixComposer.test.tsx" \
  "$studio_dir/src/renderer/components/plugins/__tests__/aixViewCommands.test.tsx" > "$result_dir/SHA256SUMS"
echo "AIX eleven-case admission, manifest/contract/port versions, dependency pins, target-isolation and service checks passed: $result_dir"
