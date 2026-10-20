---
description: Product-isolated native archive verification and upgrade
sources:
  - src-tauri/src/app_updater.rs
  - scripts/studio_delivery.py
---

# Application Updates

The native updater accepts a complete macOS application archive containing the host, frontend, selected tenant assets, ACP and Office. Bare executable updates refuse. Every declared file has a bounded size and digest; the final application signature and exact product/tenant/version identity are verified before replacement.

The existing `toyou` product uses its managed desktop release service. The new `aster` generation has no published update feed in this change and reports not published. It never requests another tenant's feed or offers that product as its own update. Linux automatic application replacement is not enabled.

## State and recovery

A single updater owns checking, downloading and installation. A recoverable failure retains the staged payload for retry. Invalid, foreign or altered packages are discarded. Different application bundles use separate staging directories. The previous complete bundle remains available during replacement; a failed relaunch command restores it.

## Packaging and verification

Build from the repository root with `just build-studio-release`, or explicitly select `A3S_TENANT_SLUG=toyou`. The package binds its frontend tenant stamp, native product ID and resource generation. Missing resources, mismatched branding and unknown products refuse packaging.

New feed publication is a separate release operation. The old 0.1.x data and update identity are not silently imported into this generation. Local fixture evidence does not claim a live published upgrade or successful application health after a relaunch.
