# Dashboard operations iteration — 2026-10-25

## Result

Implemented the first dashboard product iteration across `apps/dashboard`, `apps/api`, and `apps/studio`:

- Retired the independent Orders and Tickets destinations from the available dashboard navigation, including remembered-menu recovery. Subscription information is now an explicit section in the expanded customer detail.
- Added independent `PlatformSupplier`, `PlatformModel`, and `PlatformService` read-grant navigation, with multiple grants combined by union. Other management and Operations destinations remain directory-administration only.
- Bound the exact member account before loading organizations and isolated the SWR cache and list state by `memberId/orgId`. Conditional and inverted CASL rules fail closed instead of being widened.
- Added a typed, read-only Operations center. It shows workflow, status, subject, sequence, timestamps, error detail, and lineage without inventing a total, cursor, generic retry, or generic cancel action.
- Added `GET /v1/api/org/members/platform/operations?limit=...` to the member API. It uses the existing official-platform guard and official-org-to-Cloud-org mapping, defaults to 50 rows, and accepts only `1..=200`. Cloud remains the Operation authority.
- Allowed only the exact Operations collection GET through the Studio member proxy. No SSE, dynamic Operation path, or non-GET method was opened.

The dashboard changes are based on `dashboard@ce411f9` rather than duplicating its existing administration and audit implementation.

## Verification actually completed

- `npm run typecheck` in `apps/dashboard`: passed.
- `npm test` in `apps/dashboard`: **7 test files, 30 tests passed**.
- `npm run build` in `apps/dashboard`: passed; the CSS contract check also passed.
- `cargo check --manifest-path apps/api/Cargo.toml --tests`: passed at an intermediate revision before the final Operation subject-kind closed-set tightening.
- `git diff --check` for the edited subprojects: passed during implementation.
- Chinese and English JSON files and the dashboard package files were parsed successfully during implementation.

Tests cover remembered-menu fallback, retired destinations, resource-grant union, fail-closed access, member/org cache isolation, customer subscription presentation, Operations request/decoding, loaded/empty/error states, i18n key parity, and the ban on native `select`/`datalist` controls.

## Validation stopped by request

The user subsequently asked to finish development without further testing, so local validation paused at that point. A later deployment request required a fresh production build and the online checks recorded below; this section does not claim a complete final API/Studio regression suite.

Earlier attempts found these environment/repository issues; they were not hidden or used as passing evidence:

- API `cargo test` initially reached the Windows linker but could not open `sqlite3.lib`. A test-only bundled-SQLite dependency was tried and then removed from the final change to keep the delivery scoped to product code.
- Studio test compilation was blocked by existing Unix-only tests using `std::os::unix` on Windows. The focused Operations whitelist test did not execute. Existing tests were not deleted or skipped to manufacture a pass.
- A temporary root-workspace edit used to investigate the nested `crates/power` workspace conflict was fully restored.

## Deployment completed

- Public URL: `http://172.18.101.2:8080/dashboard` (also available through the existing `172.18.101.2.nip.io:8080/dashboard` hostname).
- `DASHBOARD_ASSET_PREFIX=/dashboard/` production build passed. HTML references `/dashboard/static/...`; no source maps, fixed server address, demo flag, or shell token was included in the uploaded bundle.
- The browser now derives the API origin from `window.location.origin`, uses the real `/v1/api/org/members/sessions` login endpoint, keeps the member token only in the current tab's `sessionStorage`, and translates desktop `/api/member/*` calls to direct `/v1/api/org/members/*` paths in browser mode.
- Published through the server's native `a3s-web deploy`: site `kense/dashboard`, release `dpl-0df8124d313c`, production alias `dpl-0df8124d313c`, managed snapshot revision 36, base path `/dashboard`, SPA fallback enabled. The existing refresh service retains the site in later snapshots.
- The API candidate was built from a copy of the server's current working tree plus only the Operations changes and the production migration registry through 139. The original remote source tree was not overwritten.
- The first API health gate failed because migration 130 was absent from the candidate registry and automatically rolled back to a healthy previous binary. After restoring the production registry through 139 and rebuilding, deployment succeeded: `a3s-api` is active, `/health` returns 200, and the deployed binary SHA-256 starts with `70caeb91b1812e8e`.
- `/dashboard` and hashed JS/CSS return 200, a missing JS returns 404, SPA deep links return the entry document, `/v1/api/org/members/me` returns 401 without credentials, the login endpoint validates an empty request with 400, and the Operations endpoint now returns 401 instead of 404. Existing `/download` and `nova` routes still return 200.
- Rollback artifacts retained: gateway journal backup `/etc/a3s/backups/main-journal-before-dashboard-20261025T025036Z-bca01811.json` and previous API binary `/opt/a3s/bin/a3s-api.prev`. Temporary upload and source-staging directories were removed.

## DESIGN.md correction and redeploy

- The first login page was rejected for not following `DESIGN.md`. Actual headless-browser screenshots confirmed that it was a temporary centered card rather than an Aster product surface. The correction is not just a claim of CSS compliance: before/after screenshots were captured and reviewed.
- The login surface is now a white Aster page with the shared horizontal logo, a 384px content column, a 14px-radius `#ececec` card, 24px title, 15px muted description, shared 36px `#f5f5f5` inputs and the shared 36px filled button. The API origin is secondary 12px metadata outside the card.
- Global type now uses `--app-user-font-family` / `--font-family` with the specified Ubuntu and Microsoft YaHei fallbacks. The platform sidebar uses the shared A3S `LogoMark` instead of a generic building icon. Existing 260px sidebar, 24px page titles, 48px table rows, 36px navigation rows and shared collection controls were retained.
- Published as `kense/dashboard@dpl-0758bf5894b2`, managed snapshot revision 37. Online CSS and JS return 200 and the deployed login screenshot matches the reviewed local version. The gateway journal backup for this release is `/etc/a3s/backups/main-journal-before-dashboard-design-20261025T033859Z-5fa55a7c.json`.
- Visual evidence: `.workbuddy/artifacts/dashboard-before.png`, `.workbuddy/artifacts/login-after.png`, `.workbuddy/artifacts/dashboard-demo-before.png`, `.workbuddy/artifacts/dashboard-demo-after.png`, and `.workbuddy/artifacts/dashboard-online-design.png`. The full platform screenshot uses the explicitly marked local demo preview; the online screenshot is the real login surface.
- A separate API deployment at 03:23 replaced the earlier Operations-capable binary and caused the route to return 404 again. Operations was then merged into the server's current source tree rather than repeatedly replacing the binary. A fresh build of that latest source passed the health gate. The current binary SHA-256 starts with `194d4cda2a115ca9`; Operations returns 401 without credentials and the final `/health` response is 200 with all checks up.

## Captcha login correction

- The reported login failure was traced in the API journal to `computer is required`, not to a bad password. The Web login now creates and persists a standards-compliant `web_*` computer id (8–128 safe characters), sends it as `computer_id`, and no longer hides that protocol error behind a generic password message.
- Added the real slide-puzzle flow: `POST /v1/api/admin/captcha/challenges`, drag verification with x/duration/trajectory points, one-time `POST /v1/api/admin/captcha/token-verifications`, then `POST /v1/api/org/members/sessions`. A failed login consumes the token as designed and automatically presents a fresh challenge.
- The component uses the actual background and piece images, including the 12px transparent piece pad when mapping visual position to the submitted x coordinate. It supports pointer and keyboard input, enforces human-paced duration and at least four trajectory points before calling the server, and exposes refresh, remaining attempts, expired/exhausted challenges, and verified state.
- Known invalid phone/password responses are localized; other server errors remain visible instead of all being labeled as password failures. The member token remains limited to `sessionStorage`; captcha tokens and passwords are not persisted.
- Final release: `kense/dashboard@dpl-75dfdb24a742`, managed snapshot revision 42. Gateway backup: `/etc/a3s/backups/main-journal-before-dashboard-login-i18n-20261025T041859Z-832c8b05.json`. Intermediate release `dpl-05a82502303a` contained the same captcha flow before the error-text localization.
- Real browser automation was run first against a local API proxy and then against the production URL. Production result: `captchaVerified=true`, `computerIdPresent=true`, and the deliberately invalid account returns the localized `登录失败，请检查账号和密码。` rather than `computer is required` or a captcha protocol error. Screenshots: `.workbuddy/artifacts/captcha-loaded.png`, `.workbuddy/artifacts/captcha-verified.png`, and `.workbuddy/artifacts/captcha-login-result.png`.
- Final API health is 200 with all checks up; Operations remains registered and returns 401 without credentials. No claim is made that a real member account completed login because no member credentials were used for the test.

## ToWork login parity

- Replaced the custom React login card with the actual ToWork reference implementation from `apps/studio/src/renderer/shell/index.html` and `studioClient.mjs`, rather than maintaining a second visual interpretation.
- The deployed page retains the original background, ToWork wordmark, QR/QR-caption column, SMS and password tabs, agreement text, captcha modal and drag handle, send cooldown, clear controls, password visibility, validation and error classification.
- The Web adapter keeps the reference state machine but replaces desktop-only runtime behavior: same-origin unified-gateway URLs, `/v1/api/captcha/*` to `/v1/api/admin/captcha/*`, one-time captcha-token consumption, `computer_id`, sessionStorage member sessions and the post-login `/dashboard` redirect. Desktop provider/model setup is not invoked in this mode.
- `scripts/prepare-login.mjs` generates content-addressed login HTML, client module, background and QR assets, plus the current login URL consumed by `DashboardSessionGate`. This satisfies the gateway's `--strict-assets` rule instead of disabling it; the first un-hashed upload was rejected before production was changed.
- Final release: `kense/dashboard@dpl-a6a9fa1cfb59`, managed snapshot revision 45. Backup: `/etc/a3s/backups/main-journal-before-dashboard-towork-hashed-20261025T044905Z-0303a170.json`. `/dashboard` redirects to the current `login.3956f2502b17.html`, so the root path and immutable cache contract both work.
- Production browser automation confirmed the ToWork UI, password-tab captcha modal, actual puzzle completion, persisted computer id, the expected `邮箱或密码不正确` error for a fresh invalid account, and SMS-code form validation without sending an SMS. A repeated test phone also demonstrated the existing account-lockout policy; a fresh invalid phone was then used for the ordinary invalid-credential assertion.
- Screenshots: `.workbuddy/artifacts/towork-login-page.png`, `.workbuddy/artifacts/towork-captcha-dialog.png`, and `.workbuddy/artifacts/towork-login-result.png`. No real SMS was sent and no real member account was used to complete login.

## Post-login organization selection

- Added `OrganizationGate` between authentication and the platform dashboard. A fresh login now sets a one-time `a3s.dashboard.login-just-completed` marker, so a member with multiple organizations is asked to choose even when an older center exists. Choosing clears the marker and writes the center for that member.
- Matches ToWork behavior: one joined organization is selected automatically; multiple organizations require an explicit choice; no joined organization shows an empty state; pending memberships are not selectable. A refresh after selection restores the center without repeatedly showing the page.
- The page reuses the ToWork background, wordmark and 36px organization rows with selected states, role labels and a filled enter action. The platform sidebar now has a switch-organization action for multi-org members. Aster's personal-center-only mode bypasses this gate.
- Final release: `kense/dashboard@dpl-59e60349305c`, managed snapshot revision 46. Backup: `/etc/a3s/backups/main-journal-before-dashboard-org-selection-20261025T051845Z-4ff70bc2.json`. The current login document is `login.aeb6d5b0bdd5.html`; `/dashboard` redirects to it correctly.
- Browser verification with mocked member/orgs APIs covered multi-org selection, refresh restoration, switching organizations, single-org auto-selection and the empty-org state. The test used API-contract fields including `is_official` and did not require a real member account. Final API health is 200 with all checks up.
- Screenshots: `.workbuddy/artifacts/orgs-select.png`, `.workbuddy/artifacts/orgs-entered.png`, and `.workbuddy/artifacts/orgs-empty.png`. No real member completed the login-to-organization flow in production because no member credentials were available.

## Explicitly not claimed

- No authenticated end-to-end member workflow or live Cloud Operation data verification was performed; the completed online checks cover static delivery, gateway routes, service health, and unauthenticated API admission behavior.
- No generic Operation detail, SSE stream, approval, retry, or cancel API was added. Lifecycle actions still belong to each subject owner and require separate permission and idempotency contracts.
- No claim is made that the final API/Studio revision passed all tests. The successful counts above apply only to the dashboard commands listed.
- No model/Supplier synchronization, fleet management, billing dashboard, or later-phase work is claimed complete.
- No dual-adapter repository proof was required or performed: this change adds no local Operation repository or second authority.
- Existing unrelated modifications in the parent repository and other submodules, including the pre-existing `apps/studio/packages/ui` deletion, were not reverted or claimed as part of this change.

## Abandoned alternatives and decisions

- Did not render Operations as another overview of unrelated health cards; the page uses the real Cloud Operation list contract.
- Did not add a second Operation store or direct Dashboard-to-Cloud connection. The member API adapts the existing Cloud owner API with typed, fail-closed decoding.
- Did not add a Cloud SDK git dependency bump just for one read. The API adapter uses the documented typed HTTP path against the existing owner contract.
- Did not infer retry/cancel URLs from `operation.id`. Future commands must use `subjectKind + subjectId` and the owning resource API.
- Did not leave the parent npm workspace link accidentally introduced into `apps/dashboard/package-lock.json`; the independent dashboard lockfile was cleaned.
