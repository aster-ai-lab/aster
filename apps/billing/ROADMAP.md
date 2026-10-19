# A3S Billing Roadmap

`apps/billing` is the usage-to-invoice authority. It records accepted usage
facts and one invoice per subscription period. Member showback stays at unit
price 0. It does not grant resources, mint keys, or price a plan.

Boundary (DP-1, decided 2026-10-20: option a — OEM commercial ledger):
`a3s-billing` is the member-plane commercial ledger. It never consumes
Cloud usage facts; the Cloud-side durable usage ledger and its authorized
attribution views (Cloud `I0.2c`) are a separate rail. Enforced by the
`billing_stays_off_cloud_stores` architecture test.

## Gates

| Gate | Status | Proof |
| --- | --- | --- |
| `BILL-1` | Landed 2026-10-10 | A repeated transaction id is a duplicate only when the subscription, metric, and quantity match. A divergent digest, including the loser of a concurrent insert, is `conflict` and leaves the outbox. A store outage is `store_unavailable` and stays queued. `bill` keeps one invoice id and includes usage through the request timestamp. The outbound row is written in the usage-ledger transaction (migration `105`) and a second connection can deliver it. The `a3s-billing` process refuses to listen when `A3S_BILLING_DATABASE_URL` is unset or empty; invoices stay in Postgres. Evidence: `docs/evidence/billing/20261010-billing-authority/OUTCOME.md`. |
| `BILL-DP1` | Landed 2026-10-20 | Decision record: `apps/cloud/docs/convergence/dp1-billing-boundary.md` (option a). Guard: `billing_stays_off_cloud_stores` refuses `a3s-cloud` / `a3s_cloud::` / `A3S_CLOUD_` in billing production source, red→green verified. Evidence: `docs/evidence/development/20261019-billing-cloud-boundary-guard/OUTCOME.md`. |
