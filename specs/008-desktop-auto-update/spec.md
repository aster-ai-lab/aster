# Feature Specification: Desktop auto-update from GitHub Releases

**Feature Branch**: none (main, per house convention)

**Created**: 2026-10-22

**Status**: Landed (2026-10-22; evidence `docs/evidence/desktop/2026-10-22-desktop-auto-update/OUTCOME.md`)

**Input**: User description: "Aster 桌面端基于 GitHub Releases 的自动更新（静态清单 desktop-latest.json + 启动/24h/手动检查 + 复用现有替换回滚机制）"

**Gate context**: `apps/studio` (product regime — root workspace, hermetic
tests, Tauri commands for Aster desktop tenants). Serves the desktop
product's update capability: today `update_channel(Aster)` returns
`NotPublished` by decision (`app_updater.rs:348`) because the only feed is
the account-gated gateway AppRelease API, and Aster is an account-free
tenant. This spec replaces the Aster feed source with the repository's own
GitHub Releases static manifest — the same pattern the CLI pair already
ships (`apps/aster/src/cli_pair_update.rs`, `latest.json`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Manual check discovers and installs a release (Priority: P1)

A user on the About page clicks 检查更新. The host reads the Aster channel
manifest `desktop-latest.json` from the repository's GitHub Releases,
compares versions, and when a newer darwin-aarch64 release exists it
downloads the archive, verifies the sha256 from the manifest, validates the
delivery manifest and the pinned codesign identity, atomically replaces the
bundle with backup, and relaunches. The About page shows the state machine
already shipped (available → downloading → ready → install).

**Why this priority**: This is the capability itself; every other story
depends on the feed and replace path existing.

**Independent Test**: `aster-studio` host hermetic tests: manifest parsing
(valid, missing platform, malformed, downgrade) and feed dispatch
(`FeedKind::GitHub` for Aster, `FeedKind::Gateway` unchanged for ToYou, and
each identity refusing the other's channel). No network in tests — the
manifest bytes and the curl invocation shape are fixtures.

**Acceptance Scenarios**:

1. **Given** a manifest whose version is newer than the running build and a
   matching darwin-aarch64 entry, **When** the check runs, **Then** the
   release is reported available and install proceeds through the existing
   `ReadyUpdate` path.
2. **Given** a manifest version older than or equal to the running version,
   **When** the check runs, **Then** no update is offered (refuses downgrade
   and no-op reoffers).
3. **Given** the manifest is absent (404) or the network is unreachable,
   **When** the check runs, **Then** the result is `update_not_published` /
   a named failure — never a crash, never a silent success.

### User Story 2 - Automatic check (startup + 24 h) (Priority: P1)

On startup and at most once per 24 hours, the app checks the same manifest
without user action. A found update only surfaces the existing About-card
state; nothing downloads or installs without the user pressing install
(first release keeps install manual; auto-install is deliberately not
claimed). The cadence and the enabled flag live in the local preference
store and the check can be turned off.

**Why this priority**: Industry-standard update reach; without it users on
old builds never learn an update exists.

**Independent Test**: `aster-studio` renderer vitest: the scheduler fires a
check on mount and honors the 24 h suppression and the preference switch;
host hermetic test for the preference read fail-closed default.

**Acceptance Scenarios**:

1. **Given** auto-check enabled and no check in the last 24 h, **When** the
   settings/About surface mounts, **Then** exactly one check request is
   issued.
2. **Given** auto-check disabled, **When** the same surface mounts, **Then**
   no check request is issued.
3. **Given** a check failure, **When** the cadence timer next fires,
   **Then** the check retries and the failure never blocks the app.

### User Story 3 - Release pipeline publishes the manifest (Priority: P2)

A workflow on a `desktop-v*` tag builds the Aster desktop bundle
(`just build-studio aster`), zips `Aster.app`, computes sha256, and attaches
both the archive and a regenerated `desktop-latest.json` to the GitHub
release. The manifest is namespaced apart from the CLI's `latest.json`.

**Why this priority**: The client is inert without a feed; the pipeline is
the feed's producer.

**Independent Test**: A hermetic python check in `tests/` validating the
manifest generator output (schema, digest presence, platform keys, version
ordering) against fixtures — same style as `tests/kylin_input_method.py`.

**Acceptance Scenarios**:

1. **Given** a build producing the zip, **When** the manifest step runs,
   **Then** `desktop-latest.json` contains the version, darwin-aarch64 URL,
   size, and sha256 of exactly that zip.
2. **Given** an existing manifest with a newer version, **When** the step
   regenerates it for an older tag, **Then** the generator refuses to
   regress the published version (fail closed).

### User Story 4 - Tenant isolation preserved (Priority: P2)

The Aster feed branch must not widen ToYou's surface: ToYou keeps the
gateway channel verbatim; Aster never queries the gateway AppRelease API;
neither identity reads a manifest addressed to the other product.

**Why this priority**: The existing code's stated invariant ("never query
the other tenant's feed") stays true under the new feed.

**Independent Test**: `aster-studio` host hermetic tests pinning the
identity→channel mapping and refusing cross-product manifests
(`ProductMismatch`).

**Acceptance Scenarios**:

1. **Given** the Aster identity, **When** a manifest for another product
   arrives, **Then** the updater refuses it with `update_product_mismatch`.
2. **Given** the ToYou identity, **When** a check runs, **Then** only the
   gateway channel is consulted, byte-identical to today.

## Edge Cases

- What happens when the manifest exists but lacks a `darwin-aarch64`
  entry? Refuse with `update_not_published` for this platform; never fall
  back to another platform's archive.
- What happens when the manifest JSON is truncated or oversized? The
  existing `MAX_JSON` cap refuses it (`update_failed`), never a partial
  parse.
- What happens when the download digest does not match the manifest?
  Existing `update_integrity_failed`; the staged payload is discarded.
- What happens when GitHub is temporarily unreachable? A named failure
  (`update_failed` / `update_not_published` mapping per existing
  `connect_fail`), no retry storm, no crash.
- What happens when the installed build is already newer than the feed?
  No offer (downgrade refusal), auto-check keeps the 24 h cadence.
- What happens when two app instances check concurrently? The existing
  `BusyGuard` serializes; the second check refuses with
  `update_in_progress`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The host MUST resolve the Aster channel to the repository
  GitHub Releases manifest `desktop-latest.json` and MUST refuse the
  channel with `update_not_published` when the manifest is absent or has no
  entry for the running platform.
- **FR-002**: The host MUST verify, in order, the manifest version (no
  downgrade), the product identity (`aster`), the archive sha256, and the
  delivery manifest + pinned codesign identity, and MUST refuse with the
  existing named states (`update_product_mismatch`,
  `update_integrity_failed`, `update_invalid_package`) on first failure.
- **FR-003**: The GitHub transport MUST reuse the providers curl pattern
  (config via stdin, no credentials in argv) and MUST keep the ToYou
  gateway transport on the loopback-only `exchange` unchanged.
- **FR-004**: The renderer MUST issue at most one automatic check per 24 h
  when enabled, MUST offer a preference to disable it (fail-closed default
  enabled), and MUST keep manual install explicit.
- **FR-005**: The release workflow MUST produce `desktop-latest.json` with
  {product: "aster", channel: "stable", version, published_at, notes,
  platforms: {darwin-aarch64: {url, sha256, size}}} and MUST refuse to
  regress an existing newer published version.
- **FR-006**: The updater MUST NOT query the gateway AppRelease API under
  the Aster identity and MUST NOT read ToYou-addressed manifests under
  either identity.

### Key Entities *(include if the feature involves data)*

- **DesktopChannelManifest** (`desktop-latest.json`): the static feed
  document; parsed in `apps/studio/src-tauri/src/app_updater.rs` into the
  existing `ReleaseMeta`.
- **FeedKind**: closed enum `{gateway, github}` mapping a host identity to
  its transport; `parse()`/`as_str()` per the house checklist.
- **Auto-check preference** (`app.update.auto_check`, boolean): local
  preference store, ACL-parsed by the existing preference schema pipeline.

## House Checklist *(mandatory — from Agents.md)*

- [x] New behavior has a small hermetic test in the owning package
      (`aster-studio` host lib + renderer vitest + `tests/` python manifest
      check)
- [x] Closed sets: every new enum has `parse()` / `as_str()`
      (`FeedKind`)
- [x] No production `unwrap()` / `expect()` without an infallibility
      comment (existing updater discipline carries over)
- [x] Internal `aster-*` dependencies stay on a path in this repository
      (no new dependencies)
- [x] Product configuration, if any, is ACL parsed by `aster-acl`
      (preference key rides the existing schema pipeline)
- [x] Evidence target: `docs/evidence/desktop/2026-10-22-desktop-auto-update/OUTCOME.md`
      when the landing is recorded

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `cargo test -p aster-studio --lib -- app_updater` green with
  the new feed/manifest/identity cases counted in the summary.
- **SC-002**: Renderer vitest for the About/auto-check state machine green.
- **SC-003**: `python3 tests/desktop_channel_manifest.py` green (schema,
  digest, version-regression refusal).
- **SC-004**: A loopback end-to-end check: a local static server serves a
  fixture manifest and archive; the real curl path fetches the manifest,
  downloads the archive, and digest-verifies it into a `ReadyUpdate`
  (downgrade, digest-mismatch and missing-manifest variants refuse). The
  install/replace/relaunch internals stay proven by the existing fixture
  suite in `app_updater/delivery_tests.rs`; the `AppHandle` glue between
  the two halves is not unit-reachable and is not claimed. Commands and
  counts land in the evidence OUTCOME.

## Assumptions

- Desktop releases are darwin-aarch64 first; Windows/Linux channels arrive
  later and are not claimed here.
- macOS notarization (Apple Developer ID) is **deferred**: not blocking,
  recorded as an open key; the pinned codesign check remains the trust
  anchor for this release.
- Ed25519 manifest signing is deferred until the first non-macOS desktop
  channel (decision D2 of the plan).
- A real GitHub release publication (the outward-facing step) is performed
  by the owner when the pipeline lands; this lane proves the client and
  the pipeline output hermetically plus the loopback e2e.

## Explicitly Not Claimed

- No auto-install: finding an update never replaces the bundle without an
  explicit user action in this release.
- No staged-rollout percentages (pure GitHub Releases cannot express them;
  a future beta channel manifest is the designated substitute).
- No delta/differential updates.
- No notarization and no Ed25519 manifest signature in this lane.
- No change to the ToYou gateway feed path.
