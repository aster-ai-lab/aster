# Official GLM error-code reference

Primary source: [智谱AI开放文档 — 错误码](https://docs.bigmodel.cn/cn/api/api-code).
Read on 2026-10-19.

The page distinguishes the HTTP status from the nested business error code.
Its table specifies:

- Business code `1113`, HTTP `429`: account arrears; recharge is required.
- Business code `1302`, HTTP `429`: account request-rate limit.
- Business code `1305`, HTTP `429`: temporary model overload.

The documented error response uses nested `error.code` as a JSON string, for
example `{"error":{"code":"1001","message":"..."}}`. The operator's reported
failure has the same shape with string code `1113`.

Interpretation for this fix: immediately retrying the same `1113` response
cannot repair account funding/resource state. The existing rate-limit retry
policy remains appropriate for the independently documented transient codes.
Only the proven `1113` + HTTP `429` combination on the existing canonical
Zhipu client is changed; other codes/shapes/providers keep their prior policy.
The page separately describes errors after streaming inference starts; those
stream events are outside this HTTP setup-response fix.
