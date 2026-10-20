# Resume retains the admitted optional transition ceiling

The final read-only count audit found two production recovery branches in
`conversation_runtime/owned_worker.rs` calling `resume_limit(32)` irrespective of
the session's admitted count policy. An unbounded ordinary/checkpoint recovery
could therefore fail after 32 actor transitions.

`FactRun::resume()` now resumes with the stored optional `self.limit` through
the existing Effect optional-limit API. Both recovery branches call it. The
explicit numeric `resume_limit(u32)` API stays unchanged, including zero-step
inspection used by approval matching. No large numeric substitute is introduced.

One regression in the existing `fact_control_tests.rs` reuses `ScriptModel` and
`ScriptTools`. It admits the actual zero-round session configuration, appends a
pending input, then invokes the new default resume path. With no transition
ceiling it completes twenty tools, twenty-one controlled model decisions and
more than 32 transitions. With an explicit three-transition ceiling it must
still fail at three and execute only one tool. The assertions await root's run;
they are not reported as passing here.

[source-before.json](source-before.json), bounded source before-images,
[owned-increment.patch](owned-increment.patch) and
[source-freeze.json](source-freeze.json) preserve the increment on top of root's
existing Core changes. Only the adjacent import/method, two recovery calls and
the owning test were added. Scoped formatting and diff checks passed (exit 0,
0.057472375 and 0.016178167 seconds). No Cargo, model service, GUI, VM or network
test was executed by this worker.

Exact filter:

```text
fact_control::tests::fact_log_default_resume_retains_admitted_unbounded_and_explicit_step_limits
```

The audit covered all `max_tool_rounds`/`max_rounds` Rust references in Core,
ACP, Work, Aster/terminal and Studio, plus Core/Effect count admission and
execution APIs. It found no remaining unguarded zero round comparison after the
child-continuation correction. It confirmed that normal fact user/confirm/answer
paths retain optional ceilings, stock/mixed compose restores absence after its
legacy numeric recipe, and explicit numeric Effect zero allowances and zero-step
inspection remain bounded. Trusted typed `AgentStepSpec` and trusted role zero
remain distinct from model Task JSON, whose zero is rejected before launch.
This is a source audit, not exhaustive runtime or platform verification.

Existing Task tests affected by the schema/fixture changes have these exact
filters (in addition to the previously frozen new boundary tests):

```text
tools::task::tests::test_task_params_schema_properties
tools::task::tests::test_parallel_task_params_schema_items
tools::task::tests::task_tool_exposes_one_compact_model_schema_and_hides_parallel_alias
tools::task::tests::parallel_task_tool_emits_terminal_event_for_aborted_child
```

The final audit also reconfirmed both hashes in the child-continuation/cancel
freeze before this increment; neither of those files changed during the audit.
Concurrent migration changes were preserved without edits.
