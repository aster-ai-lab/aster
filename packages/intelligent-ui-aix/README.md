# Intelligent UI AIX adapter

This optional distribution adapter packages the Intelligent UI authoring Skill
and a self-contained HTML message document as `a3s/intelligent-ui` version 0.1.0.
The renderer/parser library lives in [intelligent-ui](../intelligent-ui/README.md)
and does not depend on this adapter. AsterDream's built-in OpenUI renderer works
without this package being installed.

The adapter owns `a3s-use-extension.acl`, the standalone connection, CSS and
archive builder. Build tools and renderer dependencies come from the sibling
library's installed dependencies. From this directory run `npm run build` after
installing those dependencies. Build copies the model Skill and generated HTML
into disposable `stage/`, then writes `dist/a3s-intelligent-ui-0.1.0.aix` and its
SHA-256 file. Installation uses the existing AIX plugin flow; building alone does
not install a package for an account.

The standalone document speaks `a3s-intelligent-ui-connect`. It is not an AIX
workbench view/provider protocol endpoint. Packaging the renderer does not grant
OpenUI model source plugin capabilities. The archive uses ACL, contains no remote
assets or install scripts, and is not a VSIX extension.

## Verification

After building the adapter, from `apps/studio/src-tauri`:

```sh
A3S_CLOUD_INTELLIGENT_UI_CERT_DOCUMENT=/private/tmp/a3s-intelligent-ui-publication.json \
  rustup run 1.97.1 cargo test --lib capabilities::ui_projection \
  --no-default-features -- --include-ignored --test-threads=1
```

The native campaign installs the actual adapter archive, checks its verified
HTML and CSP, refuses altered/symlinked installations, and withdraws selection
on uninstall. It certifies document publication, not a generic AIX workbench
port or a full desktop WebView session.
