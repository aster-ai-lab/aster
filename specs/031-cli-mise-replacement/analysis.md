# Spec-kit consistency review

Reviewed spec, plan, tasks, constitution, and the frozen baseline before
implementation; refreshed after the initial fixes.

| Finding | Severity | File / task | Resolution |
| --- | --- | --- | --- |
| Functional requirements have producing tasks | Note | FR-001–FR-013; T001–T014 | Preserve the mappings recorded in research.md, including release-profile efficiency and the user-requested official AIX source |
| Full replacement includes experimental and host capabilities | Note | spec.md FR-009/011; matrix C17–C24 | Keep these tasks required and unchecked until evidenced |
| Wholesale upstream CLI delegation would retain TOML/updater coupling | Warning | plan.md; research.md | Reuse reviewed implementation contracts inside ACL-owning Aster modules |
| Existing global formatting differences prevent a clean workspace-wide gate | Warning | T012; format-final.log | Retain the failure and focused-format result; do not waive the final gate |
| Available execution host proves macOS only | Warning | T012 | Linux and Windows native checks remain required |
| New command execution and output-error behavior require final binary checks | Note | T003/T011 | Latest paired build and nine real CLI tests passed in build-revalidated.log / cli-env-tests-revalidated.log |
| New locks require complete source-declared membership, including before capture | Note | T004 | Installation manifests added; library tests cover missing/changed/extra files, legacy manifests, and validation before download publication |
| Lock command wiring has real CLI and paired-build evidence | Note | T004/T012 | Eleven tests pass in cli-lock-tests.log and the pair builds in build-lock.log; these predate the new version-selection slice |
| The new source dependency invalidated queued pager/Clippy graphs | Warning | T004/T012 | Old queued runs ended with E0432 for versions; current locked library checks resolve the declared package. cli-version-* and pager-version-tests.log reruns use the new graph |
| Numeric version selection has a pinned executable witness | Note | T004 | Ten fresh-state reference executions agree with both the resolver test and actual CLI execution; cli-version-tests.log has 13 passing cases |
| Current package checks have bounded coverage | Note | T004/T012 | Tool library: 83 pass and one live-network test ignored; pager command tests: 12 pass; CLI/ACP pair: build-versions.log passes before HTTP error changes. Owning tool Clippy passes; required CLI/pager Clippy still fails on 42 distinct indexing spans outside this task's changed modules |
| Full version/lock/backend compliance remains incomplete | Warning | T004/T008 | Context-dependent execution, user aliases, multi-version declarations, pre-resolved URLs and source cancellation remain required. Native archives, v3 layout locks, Node signed prebuilt installation and normal-build fixture gating are implemented; final owning checks remain separately recorded |

**Verdict**: ready to implement the remaining tasks. No NON-NEGOTIABLE violation
is authorized. No complete-replacement claim is supported by the initial slice.
