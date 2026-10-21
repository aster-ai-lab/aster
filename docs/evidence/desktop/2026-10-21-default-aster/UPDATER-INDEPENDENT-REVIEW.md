# Update delivery independent review

Read-only review of the current AsterDream / 通友租户 update path. No live feed, installer,
user profile or container was changed. This review did not rerun the owning Rust tests.
The root agent owns the recorded 26-test result and the actual signature/staging RED evidence.

## Conclusion

No additional blocking defect found in the reviewed product-identity change.

- `check_and_download` resolves the current product before `BusyGuard` or networking. Aster's
  unpublished channel refuses immediately. Release notes use the same gate. Legacy retains the
  existing `desktop` channel and API format.
- A ready download retains its exact `HostIdentity`. Retry compares identity, version, size and
  digest; installation compares the ready identity to the current identity again. A mismatch
  clears the ready state rather than installing a different product's archive.
- The signed generation retains schema 1 / `a3s-desktop`, plus an exact tenant. Installation
  validates the generation, executable paths, exact `Desktop` executable name, product name,
  display name, bundle identifier and version, then verifies the complete signature.
- The final signature covers the delivery manifest and host. The manifest hashes Info.plist,
  companions and resource bytes, excludes its own hash, and excludes the host's final signature
  bytes to avoid a circular hash. Executable-mode and quarantine changes do not rewrite these
  payload bytes. Signed-payload changes are not silently resealed by the updater.
- Staging is scoped by both the current bundle's parent and its complete basename, so the
  normal `AsterDream.app` and `Former.app` installations do not share a payload or unpack path.
- The success-only installed identity cache is process-owned and tied to the exact executable
  path. Validation errors and development/uninstalled `None` results are retried. The existing
  debug relaunch bypass remains compiled only in debug builds.

## Compatibility and limits

The current Legacy `Desktop` / `Former` package identity and schema 1 generation remain compatible.
An old Work executable named `aster-dream`, missing generation, wrong tenant or wrong product ID
is refused. This is not a migration from legacy AsterDream 0.1.x.

The existing Legacy API release metadata has no product field. It is trusted as the Legacy feed;
`available` / `downloaded` are emitted before full archive identity validation at install time.
The review therefore does not claim that a mistakenly published foreign archive could never
produce an update offer. It establishes that Aster never queries that feed and installation
refuses a foreign product. No new feed was published or enabled.

No publisher certificate, notarization, physical Kylin, native Wayland, final selected-tenant
Linux host/package, or complete new GUI generation is certified by this source review.

See [source receipt](updater-independent-source.json) for the exact reviewed bytes.
