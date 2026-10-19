#!/usr/bin/env bash
set -euo pipefail
studio_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$studio_root"
output="${A3S_CLOUD_DOCUMENT_TOOLS_EVIDENCE_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/a3s-document-review.XXXXXX")}"
mkdir -p "$output"
adapter="$studio_root/tests/e2e/document-tools/browser-adapter.sh"

# The production renderer must already listen here. This runner never stops
# another session's renderer or native shell.
curl --fail --silent --output /dev/null 'http://127.0.0.1:4325/windows/main/?workbench-applet=pdf'
./node_modules/.bin/vitest run --project renderer \
  src/renderer/pages/agents/documentTools \
  src/renderer/pages/documents/__tests__/DocumentOpenView.test.tsx \
  src/renderer/pages/documents/__tests__/DocumentsPage.test.tsx \
  src/renderer/data/__tests__/documentServiceTransport.test.ts \
  src/renderer/pages/agents/__tests__/pdfToolsJob.test.ts \
  src/renderer/pages/agents/__tests__/pdfToolsRun.test.ts \
  src/renderer/pages/agents/__tests__/pdfToolsOfficeHost.test.ts \
  src/renderer/pages/agents/__tests__/pdfToolHandoff.test.ts \
  src/renderer/pages/agents/__tests__/WorkbenchHome.test.tsx > "$output/renderer-tests.log" 2>&1
./node_modules/.bin/oxlint --deny-warnings src/renderer/pages/agents/documentTools src/renderer/data/documentServiceTransport.ts src/renderer/data/__tests__/documentServiceTransport.test.ts src/renderer/pages/documents/SaveDocumentDialog.tsx src/renderer/pages/documents/OfficeDocumentSurface.tsx > "$output/oxlint.log" 2>&1
./node_modules/.bin/eslint src/renderer/pages/agents/documentTools src/renderer/data/documentServiceTransport.ts src/renderer/data/__tests__/documentServiceTransport.test.ts src/renderer/pages/documents/SaveDocumentDialog.tsx src/renderer/pages/documents/OfficeDocumentSurface.tsx > "$output/eslint.log" 2>&1
./node_modules/.bin/oxfmt --check src/renderer/pages/agents/documentTools src/renderer/data/documentServiceTransport.ts src/renderer/data/__tests__/documentServiceTransport.test.ts src/renderer/pages/documents/SaveDocumentDialog.tsx src/renderer/pages/documents/OfficeDocumentSurface.tsx > "$output/format.log" 2>&1
a3s-test check tests/e2e/document-tools/review.acl --json > "$output/acl-check.json"
a3s-test run tests/e2e/document-tools/review.acl --browser-executable "$adapter" --json > "$output/acl-run.json"
python3 tests/e2e/document-tools/verify-download.py "$output/acl-run.json" "$output/download-pixels.json"
node tests/e2e/document-tools/verify-conversion-download.mjs "$output/acl-run.json" "$output/conversion-pixels.json"
node tests/e2e/document-tools/verify-geometry-download.mjs "$output/acl-run.json" "$output/geometry-pixels.json"
node tests/e2e/document-tools/verify-filter-download.mjs "$output/acl-run.json" "$output/filter-pixels.json"
node tests/e2e/document-tools/verify-assembly-download.mjs "$output/acl-run.json" "$output/assembly-reopened.json"
node tests/e2e/document-tools/verify-word-layout-download.mjs "$output/acl-run.json" "$output/word-layout-reopened.json"
node tests/e2e/document-tools/verify-office-artifacts.mjs "$output" "$output/office-reopened.json"
printf 'Review regression passed. Evidence: %s\n' "$output"
