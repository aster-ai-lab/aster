# Composer business acceptance audit

This suite tests the production AsterDream renderer and composer logic against the
requested Work review/provenance behavior. Open acceptance findings stay red;
they are not marked as expected failures. The dedicated Playwright config keeps
this audit separate from the existing Office regression suite.

See the [96-template acceptance matrix](../../../../../docs/plans/desktop-composer-review-trace-e2e-2026-10-15.md)
and [retained outcome](../../../../../docs/evidence/desktop/20261015-composer-review-trace/OUTCOME.md).

Run from `apps/desktop` with installed dependencies. Start the current renderer
in a separate terminal; the audit does not attach to or terminate user apps:

```sh
DESKTOP_RUNTIME_FILE=/tmp/a3s-desktop-composer-audit-absent node node_modules/@rsbuild/core/bin/rsbuild.js dev --host 127.0.0.1 --port 4537
```

Run the 19 production-function checks and 31 browser cases:

```sh
node node_modules/tsx/dist/cli.mjs tests/e2e/composer-audit/logic-audit.ts .a3s-test/composer-audit/logic
DESKTOP_E2E_BASE_URL=http://127.0.0.1:4537 node node_modules/@playwright/test/cli.js test -c playwright.composer-audit.config.ts
```

The browser service fixture publishes a configured explicit model for send
checks. It records the actual renderer HTTP submission and supplies controlled
messages/tool results for review and lineage rendering. It does not execute a
model, create real product files, or prove native behavior. Composition and
paste events are synthetic; system IME/clipboard testing remains separate.

UI-12 also requires Work's task-workspace host and all seven inspection tabs.
It waits for a docked, opaque, in-viewport panel, verifies snapshot content and
visits every tab. The controlled fixture supplies a native runtime descriptor;
a rendered tab shell alone cannot pass. No fixture token is an actual credential.
UI-25 rejects a clean-success card without an independent result. These follow-up
UI/UX checks are separate from the retained initial 30-case campaign; see the
[alignment audit](../../../../../docs/evidence/desktop/20261015-composer-review-trace/UIUX-ALIGNMENT.md).

Promoted A3S Test keyboard/option regression:

```sh
a3s-test check tests/e2e/composer-audit/composer-options.acl --json
a3s-test run tests/e2e/composer-audit/composer-options.acl --browser-executable "$PWD/tests/e2e/document-tools/browser-adapter.sh" --json
```

The local adapter needs the installed browser protocol 15 driver; the bundled
0.3.2 driver was incompatible in this environment. The suite uses keyboard
opening and samples collapsed state before checking menu removal. A CSS parent
absence assertion avoids the installed adapter returning `target_not_found`
for a successfully removed menu. The narrow case uses the native minimum
960×640 window. The browser's separate 390px case proves menu bounds only.

An optional transport probe can use an existing authorized native runtime
descriptor without printing its token:

```sh
node node_modules/tsx/dist/cli.mjs tests/e2e/composer-audit/live-plane-probe.ts <runtime-file> .a3s-test/composer-audit/live
```

It sends four short marker turns through a configured agent in sessions it
creates and archives only those sessions. It never changes an agent's model or
permissions. This is an HTTP transport probe, not a review/provenance or native
window certification. Its successful live/model and cleanup paths have not
been certified here: the native host stopped before the campaign created any
session. Invalid/external and disconnected descriptors have local checks.

The implemented host review/provenance lane has its own retained
[outcome and remaining gates](../../../../../docs/evidence/desktop/20261015-work-review-trace-alignment/OUTCOME.md).
Run its deterministic native contracts separately:

```sh
rustup run 1.97.1 cargo test --manifest-path src-tauri/Cargo.toml --lib agent_plane::workbench_ -- --test-threads=1
rustup run 1.97.1 cargo test --manifest-path src-tauri/Cargo.toml --lib agent_plane::turns::workbench:: -- --test-threads=1
```

For real configured author/reviewer models and real files, use an isolated
signed-in native test profile, its runtime descriptor, and its existing owned
test agent with `permission_mode = default`:

```sh
node node_modules/tsx/dist/cli.mjs tests/e2e/composer-audit/live-native-review.ts <runtime-file> <owned-test-agent-id> <new-evidence-directory>
```

The driver creates only its own 00/01/10/11 sessions, approves only the exact
fixture write/read, read-only verification, fixture SHA-256 and workspace
`pwd; ls -la` requests, refuses escalation, and aborts
its own turn if the admitted verifier is unavailable. Reports use exclusive
creation and include failed results. It never disables verification or changes
permission modes. The macOS lifecycle refusal remains a failed acceptance case,
even when an incomplete snapshot has the correct bytes. API results do not
replace CUA: the current background browser is unavailable and native CUA
reports `cgWindowNotFound`. Resume the real four-policy UI journeys when those
surfaces and a qualified execution node are available.

Pass a final `00`, `01`, `10` or `11` argument for an unchanged SOLO rerun.
The staged real-model repair probe uses a separate fresh session:

```sh
node node_modules/tsx/dist/cli.mjs tests/e2e/composer-audit/live-native-remediation.ts <runtime-file> <owned-test-agent-id> <new-evidence-directory>
```

It deliberately seeds `arithmetic.txt` with the wrong draft `5` while the final
contract requires `4`. The actual independent model must find the defect, the
main agent must repair and verify it, and the independent model must mark it
addressed before explicit acceptance. It checks persisted finding identities,
original reply preservation, distinct ordered byte versions, the current file,
and reopening the old bytes. No finding or repair response is supplied by a
fixture. A failure, truncated reply or invalid quote remains a failure.

The retained Linux campaign uses the existing Ubuntu evaluation image and a
non-root ACP process. It mounts only this isolated profile's workspace/plugin
directories, pins SSH host identity, forwards only the existing model gateway
on localhost:8080, and keeps the normal approval and completion gates. Native
bundle hashes are checked before setup and before/after execution. The VM and
private key are removed and the Mac test host is restored afterward. This is
API/native/model evidence; it does not qualify CUA, a different gateway origin,
other member profiles, or the missing disk quota.
