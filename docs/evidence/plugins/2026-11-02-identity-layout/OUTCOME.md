# Plugin repository identity layout

The user authorized a maintained local submodule and then requested the target
pre-release layout without legacy directory or OCI-format compatibility.
`plugins/` checks out the existing public `aster-ai-lab/plugins` repository,
tracks `main`, and is pinned by the parent gitlink. Parent Git tracks no plugin
source files individually. Unrelated dirty files and shared documentation
changes were preserved.

## Delivered

- Source: `plugins/<publisher>/<name>`; domain ownership remains explicit ACL policy.
- Recipes: `recipes/runtime/<target>.py`; operational publisher is `scripts/publish-runtime.py`.
- Catalog: `<publisher>/<name>/<version>/<channel>/<target>.json` under `registry/catalog`.
- Selected locks: `registry/locks/runtime/<target>.json`.
- OCI: `ghcr.io/aster-ai-lab/plugins/<publisher>/<name>`; domain is absent from the logical OCI target.
- Native producer, SDK publication/OCI reader, CLI cases, signing reader, docs,
  code ownership, exact-byte metadata attributes and read-only CI agree on the layout.
- Removed three withdrawn pre-release experiments from the active catalog.
  Three available component records were regenerated with native SDK preparation.
  The original AIX bytes, package fingerprints and OCI manifest bytes are unchanged.
- TUF generation 6 contains three targets. Root bytes and the pinned SHA-256
  `e1cf8be656c072939721b765eef12a597c668fe9c33cc0de2379370d3879ae9b` are unchanged.

## Verification

macOS ARM64; pinned Rust 1.97.1; `aster-use` default `extensions,mcp` features.

- Publication/domain tests: 7 passed (`cargo test --locked -p aster-use-extension --lib publication::`).
- OCI tests: 5 passed, including signed download, integrity refusal and cached offline preparation.
- Native CLI package tests: 3 passed.
- Repository catalog tests: 7 passed (`python3 -m unittest discover -s plugins/tests -v`).
- Studio runtime packaging tests: 6 passed using the freshly built CLI and hermetic assets.
- Both owning Rust packages pass `cargo clippy --locked -p aster-use-extension -p aster-use --all-targets --no-deps`.
  Existing warnings remain at `crates/runtime/src/driver.rs:4` and `crates/use/extension/src/plugin_manifest.rs:1102`.
- Three original qualified AIX archives were prepared by the new native CLI and
  uploaded through ORAS. `publication.json` retains identities and full digests.
  Repeating publication with a nonexistent ORAS executable succeeds without
  changing any catalog, lock or signed metadata bytes.
- Changed Rust files pass rustfmt. Workspace-wide formatting reports 15
  unrelated existing diff sections. The public evidence retains their path summary;
  full source-bearing output stays in ignored `target/plugins-bootstrap/workspace-fmt-layout.log`.
- Initial shared-cache compilation/executable startup stalled in macOS code
  validation. Samples/logs are retained. An isolated `CARGO_TARGET_DIR` built
  the native CLI successfully; native preparation, packaging and Clippy passed.
  The CLI unit tests also passed from a clean temporary executable copy.
- Temporary GHCR credential files were logged out and removed after upload.
- Remote validation passed in [GitHub Actions run 41362208825](https://github.com/aster-ai-lab/plugins/actions/runs/41362208825).
  A fresh isolated native registry verified TUF and inspected all three records
  exactly against the generated catalog (`native-registry-final.log`). An initial
  probe failed before inspection; its output and successful retries are retained.
- Commits carry valid local GPG signatures and DCO sign-off. GitHub reports
  `unknown_key`/Unverified for the existing signing identity; no account email
  or key registration was changed. Raw test logs retain their original trailing
  blank lines; source-only `git diff --check` passes.

## Explicitly not claimed

No desktop Release was created. These three GHCR packages remain private, so
public anonymous first-run installation is not qualified. Native Windows,
Linux/Kylin and other CPU runtime behavior was not verified. No renderer or
TypeScript behavior changed; full frontend lint/tests and the broad SDK suite
were not rerun. This change does not migrate or delete the separate OCR asset
repository; that follow-up was requested during this task and is being handled
separately after its live asset dependencies are migrated.
