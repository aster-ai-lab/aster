# Native independent review

Reviewed the updater and delivery identity callers against spec 004, using source and synthetic owning fixtures only. No real profile was read.

The updater carries HostIdentity through check, ready, download and install; it refuses install when the running compiled/installed identity differs. Aster returns NotPublished before the unscoped Legacy desktop feed is requested. Legacy retains the existing desktop channel. Signature checks bind executable, bundle ID, name, display name and version. Staging uses the enclosing bundle name, so the two declared product bundles no longer share payload files.

One concrete missing-resource defect was reported to the owners: macOS sealing and native delivery validation required INFO.md but omitted the selected logo, mark-light.png and icon.png. The maintained Python positive fixture contained only INFO.md and was accepted; such a signed generation could be installed and then fail native startup. Root owns the native manifest correction; the delivery owner owns the sealer correction. This report must be updated after their maintained negatives pass.

The shared release identity helper and renderer stamp checks use the declared two-product mapping. No additional definite identity bypass was found in the reviewed updater/packaging paths. This is source review, not package, signature, GUI or published-feed acceptance.
