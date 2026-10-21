# Shared document reader contract and typed failures

Result: the shared tool contract now explains UTF-8 text paths, and both desktop
hosts retain the existing typed document failure reasons. Focused library-test
regressions passed: **40 passed, zero failed, two ignored**.

## Confirmed contract and cause

`desktop_document_read` takes a file path, not a document-store identifier. ACP
already resolves relative paths against its bound workspace, rejects parent
traversal and lexical escape, and checks the canonical returned file URI and
workspace identity. The host repeats session, workspace identity and canonical
path checks. Shared editor documents can hold unsaved UTF-8 text; they do not
parse binary Office/PDF content. `WorkspaceDocuments::open` returns the existing
`DocumentError::Unsupported` for NUL-containing or non-UTF-8 bytes.

The Work and Desktop native turn pumps discarded every `DocumentReader` error
through `ok().and_then(Result::ok)`, then reported `document not found`. That
masked both unsupported content and the existing bounded-read errors. This is
separate from `parse_document`: its published schema already requires an
absolute path, and its implementation enforces that contract.

## Minimal changes and callers

- ACP's shared tool description now states UTF-8 plain-text/editor-model
  semantics, including unsaved text. It directs Office/PDF callers to
  `parse_document`, when available, with an absolute path. The `file_path`
  schema explains absolute/workspace-relative paths and excludes document IDs.
- Both Work and Desktop document request pumps preserve the existing typed
  `DocumentError` message. A failed blocking task maps to the existing fixed
  `Unavailable` error. Missing host, cancelled read and denied/missing resources
  retain `NotFound`. Success response shape and the existing RPC error code are
  unchanged; raw I/O errors or file content are not exposed by the error path.
- The shared bridge is enabled by the explicit CLI option or the host's
  `a3sDesktopDocuments` metadata, then registered in `session_open`. Both host
  pumps use the same request/release and snapshot contracts. Their readers,
  underlying editor model, run cleanup and scoped publishers were not changed.
- No workspace, permission, parser, Office reader implementation, schema field,
  dependency or binary distribution behavior was added.

## Verification

Commands from the repository root:

```sh
cargo test --locked --offline -p a3s-code-acp --lib document_read_contract_distinguishes_text_paths_from_binary_documents -- --nocapture
cargo test --locked --offline -p a3s-code-acp -p aster-dream -p a3s-desktop --lib document_read -- --nocapture --test-threads=1
rustfmt --edition 2021 --config skip_children=true --check crates/code/acp/src/document_read.rs crates/code/acp/src/document_read_tests.rs
rustfmt --edition 2024 --config skip_children=true --check apps/work/src-tauri/src/agent_plane/document_read_route_tests.rs apps/desktop/src-tauri/src/agent_plane/document_read_route_tests.rs apps/work/src-tauri/src/agent_plane/turns.rs apps/desktop/src-tauri/src/agent_plane/turns.rs
```

Before the production description change, the new ACP contract test failed:
zero passed, one failed, 310 filtered out, 0.03 seconds. It failed on the missing
UTF-8 capability explanation, after a 4m25s test build; see
[before-fix-acp.log](before-fix-acp.log).

The subsequent focused batch initially waited for the shared build-directory
lock, then completed with exit 0 after a 4m14s test build. Exact results from
[after-fix.log](after-fix.log):

- ACP: 16 passed, zero failed, two ignored, 293 filtered out, 9.58 seconds.
- Desktop: 12 passed, zero failed, zero ignored, 783 filtered out, 2.64 seconds.
- Work: 12 passed, zero failed, zero ignored, 753 filtered out, 2.63 seconds.

The two existing ignored ACP cases require the separately built Desktop native
document probe; their ignore conditions were preserved and they were not run.
The current source hashes are in [source-hashes.json](source-hashes.json).
Scoped rustfmt and `git diff --check` passed.

The existing native-host route fixture now additionally requests a small binary
file and an 80 KiB text file and checks the exact existing `Unsupported` and
`TooLarge` messages, without a success snapshot. Its prior unsaved-text read and
foreign-session refusal remain. Both hosts have those checks. This scripted ACP
peer exercises the host's real request pump; it is a hermetic route regression,
not a real Core/model E2E result. The binary fixture is deliberately small
non-text input, not a generated or validated XLSX deliverable.

## Explicitly not claimed

- No live host or ACP sidecar was built, replaced, restarted or operated. Cargo
  commands build only library-test executables; no CUA or model was invoked.
- No source attachment, account profile, credential, runtime ACL or full host
  log was read. The desktop operator's ongoing A07 run is separate evidence.
- No new Office/PDF parser or successful corpus delivery is claimed. The
  Bridge's temporary safe upload-stage logging was retained unchanged.
