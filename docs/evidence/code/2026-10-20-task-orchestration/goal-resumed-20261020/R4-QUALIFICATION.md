# Final repair qualification

The final candidate includes the independently qualified canonical OCI carrier
digest and human-approval deadline/stale-answer fixes. It does not replace the
R2 binary, assets or artifacts used for the recorded T032 assessment.

- [Canonical carrier RED/GREEN](runtime-carrier-canonical/OUTCOME.md): same 29
  tests pass with and without serde `preserve_order`; digest checks stay strict.
- [Approval RED/GREEN](approval-timeout/OUTCOME.md): 10 distinct tests pass,
  including valid bound answers, stale/missing answers, unbound legacy waits,
  original-owner exclusion, existing ask timeouts and unattended behavior.
- [Paired build](post-approval-paired-build.json): terminal, ACP, Office and Work,
  exit 0, 71.089 s. All 52 scoped source hashes match before/after the build.
- [Signed R4 bundle](stage-r4.json): ad-hoc strict/deep verification passed;
  `/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/AsterDream-TaskOptimization-R4.app`.
  Old installed/development public bundle files were checked and preserved.
- [Actual default Bash integration](r4-final-default-bash.json): one controlled
  provider, native Work harness, exact signed adjacent R4 ACP/Office, genuine
  Linux guest, marker and `Linux` output, exit 0, next-provider feedback and
  native terminal success. It passed in 23.256 s; zero paid calls, no timeout,
  all binary hashes unchanged and no remaining owned child processes. The
  test host was freshly compiled in 10.109 s; preparation is not task wall time.
- [Frontend/product source guard](frontend-products-final-source-check.json):
  all 25 approval regression source entries and 11 product/open-folder source
  entries still match their independently qualified manifests.

The final bundle was not launched for a second CUA or paid assessment. R2 is
still the only new actual DeepSeek Flash submission and retains its failed
31m32.931s result and 8/9 oracle score. R4's controlled integration is not an
end-to-end performance comparison and does not prove its model would choose
program, search or code intelligence tools. Those tools had no calls in T032.

The initial final-format check covers 52 source files and remains exit 1:
`crates/code/acp/src/agent_tests.rs` has formatting differences in pre-existing
spans already recorded in the earlier workspace log. Those spans were preserved.
The [follow-up scoped check](post-approval-scoped-format-followup.json) passes
the other 51 files; [the original failed check](post-approval-scoped-format.json)
and whole-workspace failures remain visible. This is not a full CI green claim.

The default 50-tool budget is unchanged. It counts failures and refusals;
budget exhaustion is still exposed by ACP as `max_turn_requests`. The default
policy and recoverable stop experience need a separate generic change and
task-set qualification, as described in
[budget analysis](MAX-TURN-REQUESTS-ANALYSIS.md). The seven later R2 parameter
refusals do not persist the precise permission predicate; their cause is not
uniquely established. See [bounded diagnosis](t032-effective-args-diagnosis.md).
Windows/Linux file-manager GUI behavior has no new runtime qualification.
