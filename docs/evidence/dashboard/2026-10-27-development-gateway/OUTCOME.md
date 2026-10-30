# Dashboard development gateway — 2026-10-27

## Result

Configured the dashboard Rsbuild development/preview server to proxy same-origin `/v1/api` requests to `http://117.162.0.179:8080`. `DASHBOARD_API_TARGET` can override the target. Production browser runtime behavior remains same-origin and no credential was added.

## What landed

- Added the `/v1/api` and `/captcha` proxies with `changeOrigin` in `apps/dashboard/rsbuild.config.ts`; the second path serves the real puzzle background/piece images rather than the frontend SPA fallback.
- Documented the default gateway and override in `README.md` and `.env.example`.
- Kept the gateway address out of `.env.local`; no local environment file or secret was created.

## Verification

- Direct remote `GET /v1/api/org/members/me`: 401, proving the member route is reachable without credentials.
- Direct remote `POST /v1/api/admin/captcha/challenges` with `{}`: 201.
- Through a temporary dashboard dev server on port 7101: page 200, proxied member `/me` 401, proxied captcha challenge 201.
- `npm run typecheck`: passed after both proxy additions.
- `npm run build`: passed, including the CSS contract.
- The temporary development process tree was stopped after verification.
- Browser network evidence initially showed challenge 201 but `/captcha/bg-*.jpg` and `/captcha/piece-*.png` returning `text/html`. After adding the resource proxy, the same resource classes returned 200 as `image/jpeg` and `image/png`, the puzzle appeared, captcha/token verification returned 200, and the browser reached the authenticated platform overview with real API-backed counts and logs. No credential value was inspected or recorded.

## Explicitly not claimed

- Remote `/health` returned 404 and is not claimed as a health endpoint on this server.
- No SMS send, complete authenticated organization workflow, or production deployment was performed; the observed authenticated platform landing is not a full workflow certification.
- The earlier `/v1/api/captcha/challenges` probe returned 404 because that is the desktop path; browser login rewrites it to the verified `/v1/api/admin/captcha/challenges` path.
