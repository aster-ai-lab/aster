# AIX Composer normalization fixture

This package contributes `desktop.composer.actions` through the canonical v3
ACL manifest. Normalize uses the default window/workspace placement. Session
Normalize declares `scope = "session"` and appears only within a conversation
UI owner. Each action uses the same isolated provider to trim received text and
replace runs of whitespace with one space.

`provider.js` uses the document-ready handshake and a private MessageChannel.
The host selects Composer protocol 1. The provider receives only request ID 1,
a draft revision and at most 64 KiB of UTF-8 text. It never receives the draft
identity, conversation identity, runtime token or host services. Each invocation
uses a fresh document and may return one replacement or refuse.

Build an immutable archive from the AsterDream directory:

```sh
node scripts/build-aix-proof.mjs /tmp/composer-proof.aix composer
```

Renderer tests execute this actual script with real Node MessageChannels. Native
tests install the ZIP, discover both Composer points and the session declaration, serve verified entry/script
bytes and refuse assets after uninstall. The retained archive can be tested
explicitly with `A3S_CLOUD_TEST_AIX_COMPOSER_PACKAGE` and the ignored native
`aix_composer_built_artifact_is_admitted` test.

These are separate package, transport and component contracts. They do not
certify hosted HTTP, WebView CSP/IPC isolation, live PG/RustFS or GUI execution.
