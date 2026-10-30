# Required release resources — isolated commit, 2026-10-30

The approved scope includes resource-completeness repairs and the build,
staging, signing and installation dependencies needed to enforce them.
The commits live on `fix/required-release-resources` in linked worktrees
inside this repository. Studio remains a private submodule; the parent
records its gitlink, not its source files.

## Result and scope

- macOS CLI packaging, installation and self-update require the complete
  sandbox alongside the Aster/ACP pair. Checksums cover the companions;
  interrupted installation restores the old pair and sandbox. An unchanged
  pair still repairs missing, changed or non-executable runtime files.
- Normal and reused release artifacts must pass installed-archive sandbox
  execution before manifest generation, release creation or upload. A model
  acknowledgement or successful CLI exit without a successful tool receipt
  cannot qualify the artifact. Draft/source checks prevent replacing a
  published release or mixing source revisions.
- Desktop sealing, native update validation and final ZIP qualification
  require a shared source-owned runtime inventory. Document verification
  checks OCR models, PDFium, ONNX, fonts and notices. Renderer validation
  resolves local entry scripts and styles and requires their actual bytes.
  Resource signing precedes sealing so the manifests describe signed bytes.
- Both macOS and Kylin producers stage the resources their validators require.
  Staged companion builds refuse stale ACP/Office bytes. Desktop publication
  validates the existing channel and the final archive before any upload;
  `--force` does not bypass archive completeness.
- Existing CLI version `0.1.2`, its four platform channels, desktop notes
  inputs and unrelated feature changes are preserved. Release shell fixtures
  use that committed version/platform contract. The execution command, model,
  nonce checks and timeouts are unchanged. The probe includes its local HTTP
  fixture so it has no dependency on another session's form-test server.
- Capture is absent from the committed baseline: there is no capture crate
  or helper producer. Its in-flight integration was excluded. This commit's
  required runtime inventory contains **23 paths**, excluding `capture-helper`;
  the original shared checkout's capture integration remains untouched.

Studio commit: `e624b4933`. It contains the delivery changes and their tests
and was created with `git commit -S --signoff`; `git verify-commit` passed.

## Isolated validation

All commands below ran from the isolated parent repository root. Counts are
read from actual logs in [checks.json](checks.json); repeated runs are not
added. **122 checks passed.** Logs retain their exact bytes in gzip files.

| Command | Passed | Log |
| --- | ---: | --- |
| `python3 apps/studio/scripts/tests/test_document_assets.py` | 6 | [Documents](documents.log.gz) |
| `python3 apps/studio/scripts/tests/studio_delivery_test.py` | 24 | [Sealing/archive](desktop-delivery.log.gz) |
| `python3 apps/studio/scripts/tests/studio_product_test.py` | 12 | [Product/package entry](desktop-product.log.gz) |
| `python3 -m unittest apps/studio/scripts/tests/sandbox_assets_test.py` | 3 | [Sandbox producer](sandbox-assets.log.gz) |
| `python3 apps/studio/scripts/kylin-flatpak/test_delivery.py` | 26 | [Kylin delivery](kylin.log.gz) |
| `python3 tests/desktop_channel_manifest.py` | 9 | [Desktop publication](desktop-manifest.log.gz) |
| `python3 tests/cli_pair_delivery.py` | 20 | [CLI installation](cli-pair.log.gz) |
| `python3 tests/cli_release_workflow.py` | 6 | [CLI publication](cli-workflow.log.gz) |
| `python3 tests/cli_sandbox.py` | 7 | [CLI staging](cli-sandbox.log.gz) |
| `python3 tests/cli_sandbox_smoke_test.py` | 2 | [Execution oracle](cli-oracle.log.gz) |
| `python3 tests/studio_checkout.py` | 1 | [Pinned checkout](studio-checkout.log.gz) |
| `/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py` | 6 | [Workspace/build staging](workspace.log.gz) |

Changed Python files passed `py_compile`; changed shell scripts passed
`bash -n`. Changed Rust files passed direct `rustfmt --edition 2024 --check`.
Both repositories passed scoped `git diff --check`.

`cargo fmt --all -- --check` was attempted and refused the baseline's missing
`apps/studio/src-tauri/src/app_updater/os_feed_tests.rs` ([output](format.log.gz)).
That module is already referenced but absent in Studio's base commit
`0555e1034c0015de99896213bb135a787612bb69`; `git cat-file -e BASE:path` also
refused it. The unrelated updater feature was not pulled into this commit.

Earlier shared-workspace runs are retained separately: **14 native updater
cases** ([log](native-updater.log.gz)) and **9 shipped CLI updater cases**
([log](previous-cli-update.log.gz)). These are prior build evidence, not
claims that this isolated branch was freshly compiled as a whole.

## Final archive probes

The committed probe and installer were used with existing archives:

```sh
python3 tests/cli_sandbox_smoke.py ARCHIVE --output-dir OUTPUT
python3 apps/studio/scripts/studio_delivery.py verify-archive Aster.zip 0.1.5 aster
```

- Local `aster-cli-v0.1.3-macos.tar.gz`, SHA-256
  `66c40e146102287f31e9e87ebb8824bcd66fe4631e0330d25d433ed062726e08`:
  installed into an isolated prefix; actual tool exit **0**, guest OS
  **Linux**, and the fresh workspace nonce returned. Driver exit **0**.
  [Result](smoke-good/result.json), [events](smoke-good/model-events.jsonl),
  [installation](smoke-good/install.log.gz), [terminal](smoke-good/terminal.log.gz).
- Official `aster-cli-v0.1.4-macos.tar.gz`, SHA-256
  `00b4c9e6b68d11060274e328d22e60d3037fc2576d20b8983a740f31be0598b7`:
  refused during installation with `macOS release is missing sandbox companions`.
  Driver exit **1**, as required.
  [Result](smoke-bad/result.json), [installation](smoke-bad/install.log.gz).
- Existing local `Aster.zip` version **0.1.5**, SHA-256
  `0730c2f7384cf0f79b3ae75608b4e18d533c605d69afe639dec8fbe7bd5b4b52`:
  accepted by the committed final archive verifier ([output](archive.log.gz)).
  It was not rebuilt or installed by this commit operation.

## Explicitly not claimed

- No push, PR, deployment or release. Published CLI 0.1.4 remains defective;
  this source commit does not repair an already published archive.
- No fresh full application build or full CI pass from this isolated branch.
  The shared-workspace frontend lint attempt also failed on existing archived
  snapshots/generated output, so a full frontend lint pass is not claimed.
- No new desktop/browser interaction campaign or Linux/Windows runtime
  campaign. Cross-platform resource refusals are covered by hermetic fixtures.
- No changes to the original main checkouts or their indexes. The original
  parent's 161 staged paths and Studio's existing in-flight work are retained.
