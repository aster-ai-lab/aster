# Billing Cloud-namespace boundary guard

| Field | Value |
| --- | --- |
| Date | 2026-10-19 |
| Task | A3S OS convergence program WS0.4 / DP-1 boundary (decision-agnostic guard) |
| Landed | `apps/billing/src/architecture.rs` — test `billing_stays_off_cloud_stores` |

## Result

Landed. Billing production source may not reference the Cloud
control-plane namespace — the closed token set `a3s-cloud` (crate
prefix), `a3s_cloud::` (module path), `A3S_CLOUD_` (environment
configuration). The guard holds under either DP-1 outcome: product
usage, if ever consumed, arrives through the member-plane projection
surface, never a Cloud namespace dependency inside billing.

## Commands and counts

- Pre-check: `grep -rn "a3s-cloud|a3s_cloud::|A3S_CLOUD_"
  apps/billing/src` — zero hits in production source (clean freeze).
- `cargo test -p a3s-billing --lib architecture` — 2 passed
  (existing DDD scan + new guard).
- Red probe: `apps/billing/src/zz_probe.rs` referencing
  `A3S_CLOUD_DATABASE_URL` — FAILED with `zz_probe.rs references
  A3S_CLOUD_`; removed — 1 passed.

## Explicitly not claimed

- DP-1 is not decided; this guard enforces the narrow invariant both
  outcomes share (no direct Cloud store/client namespace in billing).
  The full landing (boundary paragraph + README/ROADMAP note) follows
  the DP-1 ratification.
- The guard scans file text, not the dependency graph; a Cloud crate
  entering through `Cargo.toml` without a source reference is caught at
  review, not here.
