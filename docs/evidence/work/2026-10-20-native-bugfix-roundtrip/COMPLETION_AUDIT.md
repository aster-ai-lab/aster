# Eight-issue completion audit

Result: accepted on 2026-10-20 for the eight issue documents in `../issues`
and `../issues-2`, T0 capability correction, and the approved terminal policy.
Issue numbers below follow the v4 background index, not the repair-slice numbers.

| Requirement | Observed result | Retained evidence |
| --- | --- | --- |
| T0: renderer capability | Actual development origins and packaged local origin retain the intended command grants; removing/restoring the grant proves refusal/allowance | [T0 outcome](../2026-10-19-renderer-capability-t0/OUTCOME.md) |
| Issue 1: model fields | Real drawer metadata survives native-store restart; boundary rejects invalid fields | [Browser campaign](bugfix-model-browser-report.json), [29 native tests](bugfix-native-resume-providers.log), [model outcome](../2026-10-19-model-settings/OUTCOME.md) |
| Issue 2: deleted model returns | Real deletion survives supplier reselection and restart; all three default slots prevent deletion | Same model campaign and native tests above |
| Issue 3: JSON scroll overlap | Exact signed WKWebView app renders original attachment and 40,002-line fixture correctly through scroll, top/bottom and zoom/restore | [Native observations](native-json-scroll-acceptance.json), [cache root cause](bugfix-monaco-cache-proof.json), [two Monaco tests](bugfix-monaco-nonce-resume-tests.log) |
| Issue 4: completed write marked blocked | Real ACP write produces bytes, matching effect digest and `terminal: verified`; composer becomes idle; undeclared or invalid evidence cannot bypass gate | [Native campaign](bugfix-native-browser-report.json), [facts](bugfix-native-roundtrip-proof.json), [14 current attestor tests](bugfix-attestor-current-tree.log), [attestation outcome](../2026-10-19-completion-attestation/OUTCOME.md) |
| Issue 5: retrieval stops prematurely | Sixteen successful distinct reads pass old evidence threshold; failed calls consume total budget; structured failures preserve text and remain failures | [Successful reads](bugfix-no-evidence-ceiling-final.log), [failed-read budget](bugfix-failed-read-budget-final.log), [six native stop tests](bugfix-native-resume-stop.log) |
| Issue 6: vault create/name fails | Real authority creates note and folder from an absent vault; editing and Command-S persist bytes; paths remain bounded | [Native campaign](bugfix-native-browser-report.json), [13 native tests](bugfix-native-resume-documents.log), [vault outcome](../2026-10-19-vault-bootstrap/OUTCOME.md) |
| Issue 7: goal repeats stop without audit | Independent evidence cap removed; budget denial persists `forced_stop: tool_budget`; next prompt clears prior stop; conflicting end_turn cannot upgrade failure | [Stop audit](bugfix-forced-stop-audit-final.log), [native stop tests](bugfix-native-resume-stop.log), [core test target compile](bugfix-budget-final-no-run.log) |
| Issue 8: invisible form locks input | Real valid form submits changed values and continues; invalid component returns error, unlocks composer and permits another turn; ownership/revision/cancel/decline boundaries covered | [Valid native campaign](bugfix-native-browser-report.json), [invalid form campaign](bugfix-invalid-form-browser-report.json), [native recovery tests](bugfix-native-resume-forms.log), [five artifact consumers without skips](bugfix-form-native-renderer.log), [form outcome](../2026-10-19-form-recovery/OUTCOME.md) |
| Terminal policy | Actual TUI launches real ACP with default medium and explicit high; recorded argv agrees; medium session budget is 48 | [Medium TUI](tui-report.json), [high TUI](tui-high-report.json), [medium argv](default-args.txt), [high argv](high-args.txt), [effort transport](bugfix-terminal-effort-transport.log) |
| Deliverable identity and signing | Exact tested app/ACP match read-only DMG contents; final 0.1.1 ARM64 DMG accepted by Apple, stapled, validated and accepted by Gatekeeper | [Manifest](cachefix-manifest.json), [Apple log](cachefix-notary-log.json), [mounted hashes](cachefix-mounted-binary-proof.json), [DMG Gatekeeper](cachefix-gatekeeper-dmg.log), [mounted app Gatekeeper](cachefix-mounted-gatekeeper.log) |
| Cleanup | App and fixture exited zero; no owned ACP child or DMG mount remains | [Cleanup proof](final-cleanup-proof.json) |

## User decisions preserved

- Disk-content attestation installs only for explicitly enabled desktop sessions;
  the completion gate and canonical path boundaries remain enforced.
- No independent evidence-count cap; medium defaults to 48 total tool rounds.
  Exhausting budget is not completion evidence.
- Historical disabled models stay disabled and visible; another delete removes
  them. Default-model references still refuse deletion.
- Successfully rendered forms may wait indefinitely. Invalid forms immediately
  return errors and recover input. No automatic timeout was introduced.

## Rejected completion evidence

Earlier packages that still overlapped are superseded. Unit tests plus a patched
node_modules file did not prove the emitted cached renderer was fixed. Native
acceptance only passed after cache invalidation and a replacement build.
The first sixteen-read fixture used the wrong argument; only the corrected
successful receipts and contents count. Narrative-only write attempts and
aborted transport fixtures do not count as attestation passes. Historical
locked-screen/keychain audits remain retained but are resolved.

## Explicitly not claimed

The deterministic loopback model validates integration, not external model
quality or unlimited retrieval. Total tool budgets can still stop long tasks.
Full renderer typecheck, repository lint/docs and a broader core module retain
unrelated failures documented in [OUTCOME](OUTCOME.md); no full-repository green
claim is made. One ignored native test is an artifact producer executed
separately, with all five renderer consumers passing without skips.
No user installation, publication, commit or submodule landing was performed.
Final [consistency assertions](completion-consistency-proof.json) pass; the
repository docs gate retains the same 182 baseline broken links with identical
output. No required acceptance work remains within the eight-issue repair scope.
