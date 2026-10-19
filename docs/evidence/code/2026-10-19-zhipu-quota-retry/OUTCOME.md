# GLM account exhaustion stops before retry backoff

The Core Zhipu client now treats the documented HTTP 429 + string
`error.code = "1113"` combination as terminal at the first response. Both
streaming setup and non-streaming completion return the existing typed
`NonRetryableLlmError`, preserving provider, status and bounded diagnostic text.
Normal 429 and unrecognized errors retain their existing retry authority.
Production and test sources are frozen in [source-sha256.json](source-sha256.json).

## Primary source and scope

The official [智谱AI error-code reference](https://docs.bigmodel.cn/cn/api/api-code)
separates HTTP status from business error code. It documents `1113`/429 as
account arrears requiring recharge, `1302`/429 as rate limiting, and `1305`/429
as temporary overload. Its JSON envelope uses nested string `error.code`.
See the bounded [primary-source note](PRIMARY_SOURCE.md).

Only the existing canonical Zhipu provider identity is recognized. The factory
already maps `glm`, `zhipu`, and `bigmodel` to `ZhipuClient`, which sets the
canonical provider name to `zhipu`. That existing alias mapping was read and
preserved; the injected regression directly exercises the canonical public
Zhipu client, not a factory transport override. No model-name, endpoint, error
message, regex, keyword list or inferred provider classification was added.

## Minimal implementation and callers

- `llm/openai.rs`: one private `zhipu_account_exhausted` method checks canonical
  provider and exact HTTP status, then deserializes only the nested string-code
  schema. The non-streaming retry predicate excludes only code `1113`.
- `llm/openai/streaming.rs`: the same predicate excludes the same terminal
  condition before the streaming setup retry loop sleeps or retries.
- Both take the existing fatal branch and typed error constructor. Unknown
  codes, malformed JSON, numeric codes, missing fields, another status, and
  another provider keep the prior policy. No RetryConfig, delays, Retry-After
  handling, dependency, public API, provider factory or diagnostic formatter
  was changed.
- The existing Agent `llm_turn` and `llm_invoker/fact_governance` callers test
  this marker before non-streaming fallback; planning and other retry
  boundaries already recognize it. The new regression also checks the shared
  typed terminal-marker accessor. Those caller paths were read, not modified.
- `llm/openai/tests.rs` adds only the new sibling submodule registration;
  `llm/openai/tests/zhipu_quota.rs` holds the synthetic transport regressions.
  Existing effort/hard-cap changes in openai.rs were preserved completely.

## Hermetic RED/GREEN

Command from repository root:

```sh
cargo test --locked --offline -p a3s-code-core --lib llm::openai::tests::zhipu_quota:: -- --nocapture
```

Valid before-fix run: **2 passed, 1 failed, 0 ignored**, 5110 filtered, 0.05s.
The only failure was the `1113` case consuming three mocked HTTP calls under a
budget of two retries, instead of one. The transient/unknown/malformed cases
and provider/status boundary cases already passed.
[before-fix-hermetic.log](before-fix-hermetic.log)

Final run after fresh Core library-test compilation: **3 passed, 0 failed,
0 ignored**, 5110 filtered, 0.01s.
[after-fix.log](after-fix.log)

The fixture uses `ZhipuClient.with_http_client` and
`OpenAiClient.with_http_client` with an in-process error transport. Paused
Tokio time avoids real delay. It observes:

- Exact `1113`: one call for streaming and one for non-streaming, zero virtual
  backoff elapsed, typed provider `zhipu`/status 429, terminal at the shared
  Agent marker boundary. The diagnostic deliberately has opaque wording so
  code recognition cannot depend on a balance phrase.
- `1302`, `1305`, unknown string code, near-miss `11130`, numeric `1113`, missing
  code, wrong envelope, plain text and malformed JSON: three calls each on both
  transports and at least 60 virtual seconds from two 30-second Retry-After
  waits, ending as typed RetryExhaustedError with status 429/attempts 3.
- OpenAI, DeepSeek and a custom provider with a GLM model name and `1113` body
  still retry. A Zhipu 503 still retries; a Zhipu 400 remains terminal exactly
  as before.

The final synthetic run logs 30 observed transport cases and 82 total mocked
HTTP calls, including the deliberately retryable negative cases. These are
fixture measurements, not live provider requests. Raw observations are parsed
from actual logs in [test-observations.json](test-observations.json).

Four directly related existing regressions were then run individually with the
same fresh test executable, without acquiring a Cargo build lock:

```sh
target/debug/deps/a3s_code_core-c0d272b9e3c2afca --exact llm::openai::tests::nonstreaming_retry_waits_for_retry_after_and_skips_terminal_failures --nocapture
target/debug/deps/a3s_code_core-c0d272b9e3c2afca --exact llm::openai::tests::streaming_retryable_and_fatal_transport_errors_are_classified --nocapture
target/debug/deps/a3s_code_core-c0d272b9e3c2afca --exact llm::openai::tests::streaming_non_retryable_http_status_preserves_provider_error --nocapture
target/debug/deps/a3s_code_core-c0d272b9e3c2afca --exact llm::openai::tests::non_retryable_http_status_preserves_provider_and_status --nocapture
```

Each was **1 passed, 0 failed**; first 0.10s, remaining 0.00s. See the four
`existing-cached-*.log` files. Total final focused coverage is **7 passed,
0 failed**. No unrelated suite was run.

Pinned edition-2021 rustfmt checks and scoped `git diff --check` passed for both
production files, registration and new test file.

## Discarded fixture and process cleanup

The first fixture incorrectly used `LlmConfig.http_override` through the factory;
that hook is currently used only by the Cursor branch. It hit zero injected
calls and attempted default-endpoint connections that timed out, using only a
synthetic test key. This was disclosed immediately and is not counted as valid
hermetic RED or a product failure. No real credentials were used. Its log is
retained as [fixture-wiring-failure.log](fixture-wiring-failure.log). The corrected
public-client injection is the basis of every valid result above.

A redundant Cargo invocation for the four existing tests waited behind the
native v8 build. Only that owned waiting Cargo process was terminated, after
its stdout/stderr were verified against this task's exact evidence-log path.
It is not counted as a failed test run. The fresh binary checks completed
instead. No other build, lock owner or application process was stopped.

## Explicitly not claimed

- No source task, answer, attachment, artifact, corpus score, GUI, provider
  selection, real account config/ACL/key, environment or runtime argv was read
  or modified. The live A04 error was supplied by the operator.
- No native product build, deployment, restart or real-provider success after
  this change was performed by this agent. Only library-test executables were
  compiled; root owns paired v8 build and later deployment.
- This does not classify all GLM 429 codes, all quota errors, numeric variants,
  other provider bodies or errors delivered after streaming begins. The exact
  proven provider/status/string-code combination is the change boundary.
- No claim of a complete workspace test pass or of fixing the exhausted account
  itself is made. The change prevents repeated calls that need external account
  state to change before they can succeed.
