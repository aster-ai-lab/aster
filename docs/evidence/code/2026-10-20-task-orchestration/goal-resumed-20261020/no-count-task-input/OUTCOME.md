# Task input cannot request an unbounded child

`TaskParams.max_steps` is model input. With Core zero rounds now meaning no
count limit, a model-supplied zero must not replace the child's positive role
limit. This slice adds `minimum: 1` to the shared task-item schema and rejects
`Some(0)` in the shared field deserializer. Missing/null and positive values keep
their prior meaning. Trusted `AgentDefinition` configuration is unchanged.

The field boundary covers the normal TaskTool object, its compact `tasks` array,
background tasks and the parallel-task compatibility entry before launching any
child. A direct-tool regression bypasses registry schema validation and checks
six zero-input combinations. Each must return typed `InvalidArgument`; the
controlled client call count and child lifecycle event count must remain zero.
The existing zero round-trip test now expects rejection. Both schema tests assert
the positive minimum. The fast text branch in an unrelated parallel lifecycle
fixture now uses a positive one-step limit, preserving that fixture's purpose.

Only `tools/task.rs` and its existing `tools/task/tests.rs` were changed.
[source-before.json](source-before.json), [source-freeze.json](source-freeze.json)
and [owned.patch](owned.patch) preserve this additional slice separately from
the earlier Core boundary freeze.

Scoped `rustfmt --check` passed (exit 0, 0.062505416 seconds); owned
`git diff --check` passed (exit 0, 0.018501041 seconds). No Cargo or test command
was run by this worker; the root owns execution. Focused filters are:

```text
tools::task::tests::task_tool_rejects_zero_max_steps_before_foreground_background_or_fanout_launch
tools::task::tests::test_task_params_max_steps_zero_is_rejected
tools::task::tests::test_task_params_schema_properties
tools::task::tests::test_parallel_task_params_schema_items
tools::task::tests::parallel_task_tool_emits_terminal_event_for_aborted_child
```

Existing omission/positive controls are `test_task_params_deserialize` and
`test_task_params_with_max_steps` in the same owning module. Source is frozen;
test success is not claimed until the root records the corresponding run.
