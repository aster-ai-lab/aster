# Independent updater and packaged-tenant review

2026-10-21. Read the actual updater, manifest validator, sealer, package entry and tenant callers, including their maintained tests. No source change or updater install was made by this reviewer. Actual signed-package/native acceptance remains the root integration gate.

| Finding | Evidence and resolution |
| --- | --- |
| Relaunch helper deleted the backup even when `open` failed | The first reviewed `schedule_relaunch` shell ran `open` followed by unconditional backup deletion. The owner replaced this with the production `RELAUNCH_SCRIPT`: successful open removes only its supplied backup; failure moves the new generation aside, restores the old bundle and attempts to open it. The two new tests execute that same shell with a controlled real opener process, including paths containing spaces. Current source resolves this finding. |
| Linux tenant marker length was checked after an unbounded read | The first reviewed `packaged_tenant_from` used `fs::read`, then rejected length >33. Current source reads through `File::take(34)` before validating the single explicit slug, resolving the bounded-read defect. |

Current-source checks found no additional confirmed blocking defect in these paths:

- Busy ownership spans taking/installing a ready payload. The guard restores a recoverable ready generation; invalid or corrupted payloads are removed and return to idle. Ready is stored before the downloaded event. A failed remote check retains the earlier verified payload.
- The incoming version is compared to the running product version. Whole-bundle replacement and rollback carry ACP and Office with the host; an existing `.previous` is now refused instead of deleted.
- A shared bundle identifier is insufficient: validation also requires the Desktop executable name, exact product marker, tenant and requested version. Required companions, frontend document and tenant metadata must be declared; hashes are bounded, resource path traversal and symlinks are refused.
- The manifest does not hash the final signed host into its own signed resource closure. The final code signature covers the host, while the resource manifest covers the frontend, companions, Info.plist and tenant data. Final signed-bundle startup must still verify the resulting generation in the integration run.
- Packaged tenant discovery precedes checkout candidates, supplies the installed tenant without relying on a shell environment, and refuses a conflicting environment slug. Linux uses the explicit Resources marker, matching the separate Linux launcher's complete-generation verification.

The reviewer inspected the real production helper used by the new tests rather than treating an internal mocked callback as a restart result. Root-owned test logs and final signature/package receipts remain authoritative for execution counts; this source review does not duplicate or inflate them. The review does not certify physical Kylin hardware, publisher notarization or a launched application merely because `open` returned successfully.

## Success-only identity cache and debug relaunch follow-up

The reviewer re-read the final `app_updater::installed_tenant` wrapper, its actual `tenant::selected_slug` caller, the validator, `package-host.sh` and renamed `desktop_delivery.py`. The process-owned `OnceLock<(PathBuf, String)>` stores only a successful installed validation. A cache hit requires exact executable-path equality; another path invokes the real validator. `Ok(None)` and errors leave the cache empty, so a later attempt can recover. The two maintained tests use a local cache and injected validation closure, avoiding process-global test pollution: one proves one successful validation for a repeated exact path plus refusal for another path; the other proves no-result and integrity-error attempts are retried before success. This reviewer inspected these assertions and did not rerun them or claim a new execution count.

Caching this installation identity does not cache the mutable environment choice or final branding: `selected_slug` still compares the current environment slug against the validated installed tenant and refuses a mismatch. Incoming updates still run direct full manifest, executable and code-signature validation, independently of this installed-identity cache. Successful installation exits the old process; a newly launched process starts with an empty cache and validates again. This implements activation-time generation validation without repeatedly hashing the entire package in provider/tenant helper calls. It does not claim to detect external manual changes to an already active bundle while that process is running; such changes are outside the supported delivery path.

The previous symlink-based development exception has been removed. The existing `A3S_DESKTOP_APP_BUNDLE=1` relaunch exception now exists only inside `#[cfg(debug_assertions)]`; the exact string `1` is required. Release code cannot reach that exception, including with a symlink executable or this environment value. The maintained helper test covers the exact debug flag, absent/invalid values and a false debug-build argument. Source inspection of the compile-time gate, rather than that helper assertion alone, establishes the release boundary. Development relaunch continues to use checkout resources and is not evidence for release-package validation.

Both follow-up changes are suitably narrow. No additional confirmed blocking defect was found in this update/identity scope. Final signed-package startup, update/retry and native acceptance remain separate root-owned integration gates; no new package or installation was exercised for this follow-up review.



## Final native-GO source identity

At the final native GO, the reviewer refreshed the eight-file source receipt and preserved its prior cache-review identity separately. `UpdateFail` now pairs all ten existing output strings with an exact parser whose error explicitly names the refused set. The maintained contract test round-trips every variant and refuses unknown, case-changed and padded strings. The parser's narrowly scoped dead-code annotation documents today's output-only boundary; no artificial production parse/format call was added. This fulfills the repository closed-set rule without adding a new input path. The reviewer read the final parser/test and did not rerun the owner's tests.

The same final native sources were copied to the independent Linux source snapshot under the root's GO; that receipt is [ime-linux-native-source-final.json](ime-linux-native-source-final.json). Source identities describe the reviewed bytes, not a completed Linux host or signed-package acceptance.
