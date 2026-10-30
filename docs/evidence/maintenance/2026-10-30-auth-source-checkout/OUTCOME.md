# Auth CI source checkout

The backend Auth job ran a cross-product architecture scan without checking out
the pinned API submodule. Its failure was therefore missing scan input, before
the architecture rule could check the API application layer. The hermetic matrix
now checks out `apps/api` for `a3s-auth` through the existing pinned backend
composite action and its existing read-only token. Other matrix jobs, test
commands, assertions, timeouts and submodule revisions are unchanged.

## Verification

Base: `efc6afd34e78d7bd9310ed8337d77c7f394828a1`. API pin and observed checkout:
`f31b13e114d7bfddca01e108dfa0b051f132f1f9`. Native macOS arm64, pinned Rust
1.97.1. The standalone scan compiles the actual, unchanged
`apps/auth/src/architecture.rs`, with its owning package's Rust 2021 edition and
`CARGO_MANIFEST_DIR`, using `rustc --edition=2021 --test`. It does not copy its
logic into a separate test.

- Before API initialization: 0 passed, 1 failed, exit 101; missing
  `contexts/logs.rs`, matching the remote Auth failure.
- After `git submodule update --init apps/api`: the same compiled test and all
  its assertions run against the exact pin. It reaches the architecture rule:
  0 passed, 1 failed, exit 101. The remaining finding is
  `a3s-api/contexts/identity/application/org_budgets.rs` accessing SQL/ORM in the
  application layer. The source imports `a3s_orm` and executes bound SQL there;
  this is a real finding, not a missing-file error. No check was weakened.
- PyYAML parses the workflow; the added step uses the existing composite
  action, `apps/api` and the existing token expression, only in the Auth job.
- `cargo fmt --all -- --check`: passed.
- `python3.13 tests/workspace_dependencies.py`: 6 passed.
- `python3 tests/studio_checkout.py`: 1 passed.
- `git diff --check`: passed.

Raw local scan results (lossless gzip), routine check logs, source hash and
artifact hashes are retained beside this file. The Auth scan's source and compiled binary are the
same in both cases; only its declared API source input becomes available.

## Follow-ups retained

Move the API budget SQL behind its owning infrastructure/port boundary and
verify the control-plane proof before claiming the Auth gate green. The current
backend Billing failure separately requires a PostgreSQL service and
`A3S_BILLING_TEST_POSTGRES_URL`; the current Use fixture failure is in PR #208's
package migration. Neither is changed by this checkout fix.

## Explicitly not claimed

No passing Auth architecture gate, full package test, Clippy result, Linux
execution or green backend/workspace CI is claimed. This workflow change fixes
the absent input and exposes the existing API violation. No API application or
repository behavior, database schema, migration, credential, billing setting or
Cloud gitlink is changed. No new release or installed desktop verification is
claimed.
