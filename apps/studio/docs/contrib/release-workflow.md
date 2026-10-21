---
description: Build and review tenant-specific native desktop releases
sources:
  - scripts/package-host.sh
  - scripts/desktop_delivery.py
---

# Release Workflow

Build the selected product from the root workspace. `just build-desktop-release` selects AsterDream by default. `A3S_TENANT_SLUG=legacy just build-desktop-release` selects the managed tenant. The selected build logic, frontend stamp, native identity, icons and package generation must agree.

Verify the actual application signature, sealed resource hashes, companion identities and isolated upgrade behavior. Use synthetic profiles for tests; do not copy user credentials or sessions into a release or fixture.

A local package is not a published release. Publication and changing a live update feed require an explicit release request. Never point a tenant at another product's archive merely because its version number is newer.

The retained legacy source and historical evidence describe older product generations. Current application behavior is defined by the selected shared-host build.
