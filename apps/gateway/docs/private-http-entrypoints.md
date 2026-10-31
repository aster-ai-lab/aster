# Private HTTP entrypoints

On Unix hosts, an HTTP entrypoint can listen on an operator-owned Unix socket:

```acl
entrypoints "os-inference" {
  address = "unix:/run/aster-gateway/inference.sock"
}
```

Attach the existing inference router to `os-inference`. This transport uses the
same routing, credential verification, endpoint grants, snapshot expiry checks,
middleware and bounded shutdown as an IP HTTP listener. A private socket does
not replace bearer authorization. It does not enable workload network access
or provide a general egress proxy.

The parent directory must already exist, use its canonical absolute path, belong
to the Gateway process owner and have mode `0700`. Ancestors must belong to root
or that owner and cannot be writable by other users without the sticky bit.
Gateway creates the socket with mode `0600` before accepting requests. A client
therefore needs access under the same operating-system ownership boundary as
well as the route's ordinary authorization.

Relative paths, path aliases, TLS, TCP/UDP protocols and trusted forwarded
headers are rejected for this transport. Unsupported Unix sockets are also
rejected during configuration validation on other platforms. IP-specific HTTP
middleware receives `unix`, which cannot satisfy an IP allowlist. Access logs
also record `unix`; upstream forwarding uses `unknown` rather than inventing
a loopback address. IP-based rate limiting shares one Unix-client bucket;
inference credential limits continue to use the authenticated credential.

The listener never unlinks an occupied path, including a stale socket. Stop the
owner and verify socket ownership before operator recovery. Graceful shutdown
on `SIGINT` or `SIGTERM`, listener cancellation and startup rollback remove only
the device/inode created by that listener. Existing requests drain within the
configured shutdown timeout. Reloading route policy keeps the bound socket;
an invalid transport policy leaves the active configuration intact.

Management and Managed Runtime Service URL contracts retain their existing IP
listeners. A Box guest-to-host binding and OS-issued workload credential are
separate requirements; adding this listener alone does not complete the
PaperLoom service's OCR integration.

Verification:

```sh
CARGO_BUILD_JOBS=2 rustup run 1.97.1 cargo test --lib unix_
CARGO_BUILD_JOBS=2 rustup run 1.97.1 cargo test --test unix_http
CARGO_BUILD_JOBS=2 rustup run 1.97.1 cargo test --lib \
  entrypoint::platform_model_tests::paperloom_gateway_tests::paperloom_private_unix_gateway_runs_real_ocr_with_endpoint_authorization \
  -- --exact --ignored --nocapture
```

Run from `apps/gateway`. The last command requires the installed platform OCR
runtime; it runs actual PP-OCRv6 recognition rather than a successful fixture
response. The real-process test requires `lsof` and signals only its own child.
