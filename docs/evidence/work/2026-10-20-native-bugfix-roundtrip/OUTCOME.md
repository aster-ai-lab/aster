# Eight-issue repair acceptance

Status: accepted for T0, all eight reported issues, terminal effort/budget policy,
and the final macOS ARM64 package. See [completion audit](COMPLETION_AUDIT.md)
for the requirement-to-evidence mapping and [final gate status](FINAL_GATE_STATUS.json)
for the machine-readable result. No task acceptance gate remains open.

## Executed campaigns

- `bugfix-models-20261019`: production renderer, native provider and Preference  
  stores, loopback model supplier. Drawer metadata survives a complete fixture  
  restart. Deletion survives provider reselection and another restart. Native  
  defaults, including all three device Preference slots, prevent deletion.  
  Final provider rerun: 29 passed. The last deletion screenshot was taken after  
  onboarding returned to Work; the actual absence assertions and restart state  
  are the deletion evidence.
- `bugfix-native-flows-retry-20261020`: the real local-document authority creates  
  a file and folder in an initially absent vault. Command-S persists the edited  
  note. The fixture forwards ETag and streams document changes; early fixture  
  attempts missing those transport details were aborted and are not passes.
- The same campaign drives real core/ACP tool calls. Approved `write` produces  
  actual bytes and a persisted `turn.complete` with `terminal: verified` and an  
  effect digest. The fixture needs its own Git root because its parent ignores  
  target/. The earlier narrative-only attempt is retained, not counted as an  
  attestation pass.
- `ask_user_form` renders a valid OpenUI card; a changed answer is submitted to  
  the native tool, the next inference completes, and the composer returns idle.  
  The submitted card is also visible in the signed native WKWebView.
- `bugfix-invalid-form-20261020`: actual core refuses an unknown OpenUI component  
  with a failed tool receipt, then the model continues. The composer unlocks  
  and a subsequent user turn settles successfully. No form remains parked.
- Actual native form/recovery artifacts were generated. Their two renderer  
  consumer suites pass all 5 cases without skips. Typed error, decline,  
  cancellation, ownership and revision refusal tests cover recovery boundaries.
- Structured terminal transport campaign: 6 passed, including read-only legacy  
  stop, tool budget, truncation, cancellation and a clean subsequent prompt.  
  A later audit also caught a conflicting `end_turn` incorrectly overriding  
  structured budget/truncation failure. Its new test fails before the fix; the  
  actual native pump now refuses both conflicts and still clears the next turn.  
  Cancellation retains priority. Core audit confirms `forced_stop: tool_budget`  
  in the retained completion fact.
- Sixteen distinct real reads complete without the removed 12-evidence stop.  
  Medium's shared limit is 48 tools. Actual TUI campaigns pass with medium by  
  default and an explicit high override. Both launch the real ACP binary via a  
  recorder, show the selected effort, complete a model response and leave no  
  cleanup error. Recorded argv independently confirms medium/high forwarding.  
  The native session test also checks the 48-round limit and a later switch.  
  CLI parser tests: 8 passed.

## Additional budget audit

The strengthened read-source regression exposed a fixture error in the earlier  
16-call test: it submitted `path` instead of the read tool's `file_path`, so its  
call-count assertion did not prove that evidence was read. The corrected test  
explicitly authorizes read and checks all 16 successful receipts and their  
file contents. Final compiled SOLO rerun: passed. The failed-read test authorizes  
read, submits three missing-file reads, checks execution-error output, denies  
the fourth call and reports `tool_budget`: passed. The separate stop-audit SOLO  
check verifies the retained `turn.complete` reason: passed. The affected core  
lib test target compiles (`--no-run`). Final logs are retained beside this file.

A broader 18-test module run had 17 failures (retained log), mostly fixtures  
rejected before inference by the default permission/presentation boundary  
(`unknown Tool 'Skill'`). The new read tests and the affected budget-audit test  
now have explicit permissions. No module-wide pass is claimed.

Concurrent attestor failure-accounting tests initially blocked compilation with  
a nonexistent `crate::sha256` reference. The compatibility correction reuses  
the existing `sha256` dependency; all five cases pass, including failed changed  
bytes, per-path recovery, replay/cache retention and failed deletion. These  
parallel safety changes landed in the working tree after the guard candidate,  
so a replacement candidate was rebuilt before final native acceptance.

## Package gate

The first signed 0.1.1 package was notarized but reproduced overlapping lines in
the original issue JSON, so it remains retained as a superseded attempt. Native
CSP needed explicit style-attribute admission for Monaco's positioned lines and
the document nonce on Monaco's dynamically created style elements. The native
configuration and actual Monaco stylesheet tests fail before their fixes and
pass after them. No script-source, network-source, Tauri capability or file
authority grant was changed by the style correction.

A later native rerun still showed transparent overlapping sticky rows. The
installed dependency was patched, but Rsbuild's persistent cache reused a chunk
without the nonce patch. The minimal final correction adds pnpm-lock.yaml,
pnpm-workspace.yaml and the pinned Monaco patch to buildCache.buildDependencies.
The warm-cache rebuild now emits the nonce assignment; the before/after asset
and hashes are retained in [cache proof](bugfix-monaco-cache-proof.json).
Scoped config format/lint and the two Monaco tests pass.

The exact signed replacement app passes native WKWebView acceptance for the
original 117.8 KB issue attachment and a 10,000-record, 40,002-line JSON file:
scroll down/up, ordered content, opaque sticky rows, last record, first record,
panel maximize and native window zoom/restore. The original screenshot's
ask_user_form region was directly rechecked without overlap. See
[native observations](native-json-scroll-acceptance.json); screenshots were
inspected in the CUA conversation, not saved as new PNG evidence.

Final artifact:
`target/packages/asterdream-bugfix-20261020-cachefix/aster_0.1.1_aarch64.dmg`.
Version: 0.1.1, bundle dev.a3s.studio, macOS ARM64.
Post-staple SHA-256:
`b3ee4427fa21666dbaa4185e914836c60598d245ea3ed6b4ac76e8e108d4bad3`.
Apple submission `2fe2cd71-abd8-403c-9770-9ff0b35aebbc` is Accepted with no issues;
the submitted pre-staple hash matches Apple's log. App and DMG tickets are
stapled and validated. Deep strict codesign and DMG integrity pass after
stapling. Gatekeeper accepts the DMG, built app and mounted app as
Notarized Developer ID. Both application and ACP executable hashes inside the
read-only mounted DMG match the native-tested manifest. See
[final manifest](cachefix-manifest.json), [Apple log](cachefix-notary-log.json),
[mounted binary proof](cachefix-mounted-binary-proof.json) and retained
cachefix-prefixed validation logs.

The test app and fixture server exited successfully, owned ACP children are
gone, and the DMG is unmounted. [Cleanup proof](final-cleanup-proof.json)
records zero owned leftovers. Earlier acceptance/guard/final packages are
superseded and must not be delivered as the final fix.

CSP diagnosis follows [Tauri CSP](https://v2.tauri.app/security/csp/) and the
[style-src-attr contract](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/style-src-attr).
The cache correction uses Rsbuild's existing
[buildDependencies option](https://rsbuild.rs/config/performance/build-cache).

## Final consistency check

The final DMG hash, Apple submission identity and pre-staple hash, tested native
binary, five browser/TUI reports and cleanup proof agree. All 66 local evidence
links in the final records resolve; all eight issue documents now link their
scoped acceptance. The Work config/ROADMAP diff whitespace check passes.
`pnpm docs:check` still finds 182 broken links: its complete output is identical
to the prior retained baseline, with no new failure. See
[consistency proof](completion-consistency-proof.json) and
[final docs log](bugfix-docs-final-check.log).

## Explicitly not claimed

The loopback LLM supplies deterministic responses; it is not a live external  
model-quality test. Full renderer type checking and the read-only lint/docs gates still report  
unrelated broad working-tree failures. The 19,404-entry i18n check passes. No user-system installation, publication, commit or  
submodule landing has been performed. Unrelated in-flight changes are retained.

## Historical external prerequisite audit (resolved)

At 2026-10-20T01:33:32.147236+08:00, the exact candidate DMG, application, ACP binary, Monaco patch and four key production sources still match their retained SHA-256 values. The OS still reports `IOConsoleLocked = Yes`, and the saved `AsterDream-notary` profile lookup fails. These equivalent blockers have recurred across three consecutive goal turns. No owned build or notarization job is pending. Native capture requires local screen unlock; notarization requires the saved profile to become available or be restored locally. At that audit the full objective remained unachieved. On resumption the screen was unlocked and the saved notary profile was available; the final cachefix package subsequently passed native acceptance and the full notarization chain described above. These prerequisites are no longer blockers.
