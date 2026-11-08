# Aster SMS

Provider-neutral Rust SMS ports, an always-available deterministic virtual
provider, and an opt-in SMSBao adapter. Cloud now owns scoped profile
configuration, exact secret binding, HMAC idempotence, durable one-attempt
sending and redacted records. Console exposes the service under System →
Security & sign-in → SMS. See the [operator guide](../../apps/cloud/docs/sms.md).
External delivery receipts, further native suppliers, Flow/member consumption
and commercial Billing remain separate roadmap work.

`SmsProvider` is object-safe and injected explicitly. `prepare` validates
locally and binds its consumed handle to the exact provider instance and
profile revision. `submit` distinguishes accepted, definitely not accepted
and indeterminate. Acceptance is not handset delivery. The caller owns durable
attempt IDs and must not automatically retry an indeterminate submission.

Enable the `smsbao` Cargo feature to use `SmsbaoConfig::from_acl`,
`SmsbaoProvider::from_config` and an explicitly injected
`SmsCredentialResolver`. The [ACL example](examples/smsbao.acl) contains only
versioned secret references. Supply a real approved signature before sending.
The adapter supports one mainland-China mobile recipient and text content;
template, delivery receipts, status lookup and native idempotency are absent.

SMSBao uses HTTPS GET with URL-encoded content. Password authentication derives
a lowercase MD5 in memory; `password_md5` accepts a prehashed value, and
`api_key` passes the key unchanged. No redirects or HTTP retries are enabled.
Responses and deadlines are bounded; transport loss or an unfamiliar response
remains indeterminate. Errors and Debug omit credentials, content and raw URLs.

The response format is explicitly configured (`txt` or `json`); formats have
separate error-code mappings. The example uses `txt`, verified by the live
read-only balance check. The documented JSON balance format did not produce a
recognized response during that check; JSON is covered by fixtures, not a live
success claim. There is no automatic format fallback or resubmission.

Read-only diagnostic (two secret lines supplied through stdin):

```sh
rustup run 1.97.1 cargo run --manifest-path crates/sms/Cargo.toml \
  --features smsbao --example smsbao_balance -- crates/sms/examples/smsbao.acl
```

The diagnostic never sends SMS and prints no balance or credentials. Use a
secret manager or a non-echo stdin wrapper. Product configuration is ACL only,
parsed by `aster-acl`; JSON here refers only to the external API response.

Protocol sources: [domestic API](https://user.smsbao.com/docs/developer/domestic-sms-api/)
and [error codes](https://user.smsbao.com/docs/developer/error-codes/).
See the [roadmap](ROADMAP.md) and
[retained evidence](../../docs/evidence/sms/20261008-smsbao-provider/OUTCOME.md).
