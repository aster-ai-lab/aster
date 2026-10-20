# Preserve an explicit role limit equal to the SDK default

The Task caller applies an agent definition, then parent context. Both existing
APIs recognize the numeric SDK default (50) as an unset count. An explicit role
`max_steps=50` therefore became zero under an unbounded parent, and its explicit
limit was lost. The earlier role-three/role-seven controls did not expose this
numeric alias.

A small private Task helper now reapplies a known positive role limit after
parent inheritance only if the resolved count is zero or wider than that role
limit. Parent zero/role fifty resolves to fifty; parent three/role fifty remains
three; parent 3200/role fifty resolves to fifty. Trusted role zero and absent role
limits are left unchanged. The existing later positive TaskParams override stays
in its existing position. This is a targeted role restoration, not a general
silent clamp or a replacement for capability admission checks.

Two unit regressions exercise the same real `AgentDefinition.apply_to`,
`ChildRunContext.apply_to` and private-helper chain used by the Task caller.
Their tables cover the three fifty-role counterexamples, ordinary role three,
and absent/trusted-zero roles with absent, zero, three and 3200 parent counts.
No provider, child execution or extra fixture framework is required.

Only the already owned Task source and tests changed. The previous input-zero
freeze remains intact. [source-before.json](source-before.json), the bounded
source before-images, [owned-increment.patch](owned-increment.patch) and
[source-freeze.json](source-freeze.json) preserve this additional increment.
Scoped formatting and diff checks passed with exit 0; their exact durations and
commands are recorded in the freeze. No Cargo/test execution was performed by
this worker. Root validation remains required after the unrelated parallel
porcelain edits compile coherently.

The exact new filters are:

```text
tools::task::tests::task_positive_role_round_limit_survives_parent_default_alias
tools::task::tests::task_role_round_tightening_preserves_absent_and_trusted_zero_roles
```

Retain the prior direct-zero TaskTool and schema regression filters from
[the input-boundary slice](../no-count-task-input/OUTCOME.md). This source freeze
does not claim a completed compile or GREEN run.
