---
description: >-
  Preference architecture - generated key schema, native ACL storage, BootConfig
  routing, renderer cache, and cross-window sync
sources:
  - src/renderer/data/PreferenceService.ts
  - src/renderer/data/hooks/usePreference.ts
  - src/shared/data/preference
  - src-tauri/src/preferences.rs
  - src/renderer/shell/desktopMainBridge.js
  - scripts/test-native-preferences.sh
---

# Preference System Overview

Preference stores small, fixed-key user settings that must persist and converge
across windows. It is not for user-created records, large collections, or
regenerable UI state.

## Contract

- Ordinary keys and defaults are generated in
 `src/shared/data/preference/preferenceSchemas.ts`.
- `PreferenceKeyType` covers ordinary stored keys.
- `UnifiedPreferenceKeyType` also includes public BootConfig keys under the
 `BootConfig.` prefix.
- Keys have generated UI defaults. A failed native read must not activate a
 Harness recipe; capture checks the confirmed renderer cache.
- Keys are fixed by the schema; users modify values, not the key set.

The adding-key workflow is documented in
[Preference Schema Guide](./preference-schema-guide.md). The generated file is
not edited directly.

## Storage and Routing

The Tauri host stores ACL under its platform application-data directory,
partitioned by launch tenant/instance and the member UUID confirmed by the
native member session. The renderer cannot select a different member on reads.
Commits bind that member, the expected revision, and the generated key set.

| Key | Store |
|---|---|
| Ordinary generated key, such as `ui.theme_mode` | Member-scoped native ACL |
| Public `BootConfig.app.*` key | Seeded read; native write routing remains pending |
| Internal `BootConfig.temp.*` key | Not admitted by the native Preference store |

A file lock serializes readers/writers across processes. An ordinary
`setMultiple` validates the whole batch, checks its revision, writes and syncs a
private temporary file, then atomically replaces the destination. Conflict
retries apply only the caller's requested keys. A batch containing a BootConfig
key is refused before writing any ordinary key.

## Process Responsibilities

### Native host

`preference_read` and `preference_commit` check the main label/document and the
existing host token installed only in the top-level owned renderer.
Blocking filesystem work runs off the UI thread. The member-session lock keeps
identity stable through each store operation. Successful commits notify the
main WebView with an empty event; values are fetched through a fresh native
read. Notifications do not carry settings or credentials.

### Renderer

The renderer singleton caches values lazily. A read fetches an uncached key from
the production bridge and establishes the matching subscription. Native notifications trigger fresh reads;
the renderer updates its cache and notifies hook subscribers.

`usePreference` and `useMultiplePreferences` use `useSyncExternalStore`, expose
generated defaults while an uncached value loads, and return Promise-based
setters.

## Update Strategies

Renderer writes are optimistic by default:

1. Update the local cache and notify React.
2. Persist through Main.
3. Confirm on success or restore the protected original value on failure.

With `{ optimistic: false }`, the renderer waits for Main to confirm persistence
before changing its cache. Use this when the UI must not present an unconfirmed
setting as saved.

Batch operations apply the same choice to every key in the batch. The store skips
unchanged values and publishes notifications only after successful writes.

## Data-System Boundary

| Data | Owner |
|---|---|
| Theme, language, feature toggles, shortcuts | Preference |
| Process flags needed before lifecycle | BootConfig, exposed later through `BootConfig.*` |
| Regenerable window/UI state | Cache |
| Providers, conversations, notes, other business rows | DataApi/SQLite entity services |

API credentials attached to providers are provider business data, not a generic
Preference key.

## Debugging

Run `bash scripts/test-native-preferences.sh <new evidence directory>` after
installing the pinned AsterDream dependencies. It verifies real native storage and
the production bridge over test stdio, including the real renderer cache and
Harness capture. Its controlled event transport does not certify Tauri event
delivery or native WebView isolation.

The browser-only fixture retains its existing localStorage adapter. Native
startup does not import unscoped legacy choices into a different member's
profile: old values remain, and Harness capture refuses an unresolved legacy
choice until an explicit new choice commits. Corrupt ACL is preserved for
diagnosis and refuses both reads and writes. Real WebView multi-window/restart,
session-transition delivery, and BootConfig write routing remain acceptance
work under T005; the full Meta AsterDream gate is still open.

## Related Documentation

- [Preference Usage](./preference-usage.md)
- [Preference Schema Guide](./preference-schema-guide.md)
- [Boot Config Overview](./boot-config-overview.md)
