# Controlled provider accepted-socket correction

The final-binary-v3 five-case run remains recorded as **4 passed / 1 failed**.
Its refused-approval case reached a successful terminal with approval denied and
no tool execution, then failed the controlled-provider background extraction
drain. The ACP logged a loopback connection reset before that drain expired.
This correction changes test fixtures only; it does not recertify that run.

## Cause and boundary

`Provider::serve` makes its listener nonblocking so its thread can poll for
shutdown. It previously read each accepted socket without explicitly making
that socket blocking. On Darwin, `accept` inherits the listener's file flags:
[Apple XNU accept implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/uipc_syscalls.c)
copies the listener flags to the new descriptor and synchronizes `SS_NBIO`
from `FNONBLOCK`. The pinned Rust 1.97.1 macOS implementation calls
`libc::accept` and only adds close-on-exec. Apple's
[accept(2) reference](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/accept.2.html)
also describes the new socket as having the same properties as the listener.

The new small regression uses a real nonblocking loopback listener. It signals
that the connection was accepted before the client withholds all request bytes.
The old parser returned `Resource temporarily unavailable (os error 35)` before
any request was sent. Its existing `?` would exit the complete provider thread,
closing the connection and listener. This reproduces the fixture defect that
can produce the historical reset; the historical run's original server error
was suppressed during panic, so its precise first error cannot be recovered.

`requests` is updated only after successfully writing a response. Therefore
`provider_aux_requests=0` does not prove that ACP failed to start extraction.
The historical ACP explicitly logged its extraction HTTP attempt and failure.
Core's memory extraction error branch records `ProviderFailed` and calls
`attempt.finish()` (`memory_extraction_runtime.rs`), so these observations do
not establish a production resource settlement leak or an old ACP mismatch.

## Change

The existing bounded parser is factored into `Provider::read_request`, used by
the server and the regression. Each accepted socket now explicitly uses
blocking I/O, retaining its five-second read timeout and adding a five-second
write timeout. Errors identify the I/O stage, error kind and bytes received.
EOF, request size, HTTP path, Content-Length, JSON and auxiliary-request
validation remain strict. Unexpected HTTP diagnostics omit raw headers.

`Provider::Drop` now prints a failed thread result even during another test's
panic, and retains the original normal-path assertion. The refused-approval
assertions, program fixture, background extraction and three-second drain are
unchanged. No empty-connection exception or retry was added.

## Checks

- RED: the two socket regressions ran with the original parser behavior;
  partial EOF was refused, while delayed first bytes failed with macOS error
  35. Result: **1 passed / 1 failed**, exit 101. See [red.log](red.log).
- GREEN: the same two tests passed. The successful request waits across both
  the 250 ms first-byte delay and a further 50 ms body delay; a partial request
  ending in EOF remains refused. Result: **2 passed**, exit 0, 0.32 seconds
  test execution. See [green.log](green.log).
- Scoped rustfmt check: edition 2024, `skip_children=true`, exit 0.

Exact commands and counts are in [checks.json](checks.json), frozen source
digests in [source-final.json](source-final.json), and the original provider
source is retained in [delivery-before.rs.txt](delivery-before.rs.txt).

## Explicitly not claimed

No ACP, model, VM, application or GUI was started by these checks. No production
source or application binary was replaced. No historical result or paid sample
was changed. No claim is made that the original reset's exact server-side errno
was captured, that the five final ACP cases now pass, or that observing the
provider response alone proves every memory retention ticket has settled.
The new Work test harness still needs to run the unchanged five-case protocol
qualification against the frozen ACP identity before advancing that gate.
