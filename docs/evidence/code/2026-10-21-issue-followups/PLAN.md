# Remaining issue delivery

Continue after PR #8: fix the failing CI checkout/toolchain/install paths and the missing tenant source; reproduce the headless API-key hang using bounded local fixtures; wire full prompt replacement without removing host permission contracts; deliver a validated CLI/ACP pair.

Work stays private and pinned by the root gitlink. CI access uses a dedicated read-only deployment key if needed, never an existing user credential. Preserve the original dirty main, use the existing isolated checkout at the merged base, and stage only reviewed own changes. No guard is removed to make checks pass.

Retain commands, package identities and before/after evidence. Source fixes, CI completion and published installers are distinct claims; finish and verify each before claiming completion.

The initial repository-scoped read-only deploy-key option was refused by
GitHub policy (Work HTTP 422); organization secret inventory is also denied
(HTTP 403). No deploy key or secret was created. The concrete CI action now
uses `CI_SOURCE_APP_ID` / `CI_SOURCE_APP_PRIVATE_KEY`, with a separate
contents:read token scoped to each pinned private source checkout. This
requires the owner to configure a dedicated App for work/api/cloud.
