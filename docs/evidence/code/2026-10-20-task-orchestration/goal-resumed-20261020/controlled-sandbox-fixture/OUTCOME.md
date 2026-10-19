# Controlled Work → ACP → Core default Bash sandbox fixture

Status: **root-owned actual default Bash / Linux guest smoke passed** against the new R3 companion containing the SDK canonical-path repair. [Run result](../t031-r3-default-bash.json) and [complete log](../t031-r3-default-bash.log) record exit 0, **1 passed**, test time **21.74 s**, and whole-process wall time **23.66289425 s**, including the fixture's finally cleanup. This controlled loopback run used **0 paid model calls**. The original R2 live CUA T01 assessment was not touched or replaced.

Only two test files changed:

- `apps/work/src-tauri/src/agent_plane/local_acp_delivery_tests.rs`: two lines declaring the new sibling module. Removing exactly those two lines reproduces its saved baseline hash.
- `apps/work/src-tauri/src/agent_plane/local_acp_sandbox_tests.rs`: one ignored macOS controlled-provider fixture.

The original grep test, five program scenarios, and provider helper retain their baseline SHA-256 identities. This fixture change did not modify production code, policy, dependencies, sandbox assets, or application/user state.

## Executable and assessment boundaries

- [R2 stage](../stage.json) and its [actual port-4319 CUA launch](../stage-launch-4319.json) identify the original paid T01 assessment. Its ACP SHA-256 is `61a06ab66f5f43ea89b2d1a909da5ef146bef2802b9526e75e8792a61d9e09f7`. That run remains separately measured; this R3 pass does not rewrite its earlier carrier refusal or establish its terminal outcome.
- [R3 stage](../stage-r3.json) is the separately signed, strict-verified post-canonical build. Its GUI application was **not launched**, and no additional paid CUA submission was made for R3.
- The controlled smoke ran the root's frozen Work libtest `target/qualification/t031-r3-cua-20261020/work-tests`, SHA-256 `fb2efde6fa4dad819991821194be5ad37470d27fb1bd74c6a8fa304e064c8aa0`. Its sibling ACP is a frozen copy of the signed R3-stage ACP, SHA-256 `7df56d80d617722e4e3afa92335c0a853fd6ce7cb6b5e3d3b8c889aa6edabeb2`. [Frozen clone identities](../t031-r3-binaries.json) and the run result bind these exact binaries and record all three clone files unchanged afterward. This exercises the native Work test harness plus real ACP/Core, not the R3 GUI executable.

The root runner observed ACP **54900**, its OCI Host **55035**, and shim processes **55051 / 55052** using the existing qualified hardware assets. It recorded **`owned_processes_remaining: []`** after the test. The fixture PASS marker was printed after its own cleanup; the separate runner observation additionally covers the native descendants. This establishes the controlled guest/carrier result and owned-process settlement, not T01 correctness or a model-speed improvement.

## Exact runner handoff

Filter:

```text
agent_plane::turns::local_acp_tests::delivery::sandbox::real_acp_default_bash_runs_in_linux_guest_and_reaches_provider
```

```text
ASTER_SANDBOX_DIR=<existing-qualified-hardware-assets> <fresh-work-libtest> --ignored --exact <filter> --test-threads=1 --nocapture
```

The fixture requires the runner's existing `ASTER_SANDBOX_DIR` and checks the required companion/image/bundle paths exist. This run used `target/t013-assets-hardware`; it launched that qualified Host/shim without rewriting the retained assets. `A3S_CODE_ALLOW_PROCESS_HOST_SANDBOX` must be absent; the fixture refuses that fallback override. Binary selection remains the existing `workspace_acp()` sibling lookup, with actual mapped executable and unchanged SHA-256 assertions.

The trusted temporary host ACL selects adaptive presentation and disables workspace retrieval; its guard restores the previous `A3S_CONFIG` path. The local synthetic model is `openai::gpt-test` at the existing controlled loopback provider. No real provider credentials or service are used.

## Verified behavior

The sole scripted tool call is `bash` with exactly one argument:

```text
{"command":"printf '<unique marker>\\n'; uname -s"}
```

There is no `sandbox_permissions`, justification, grant, or escalation argument. The actual presented Bash schema must declare the default `use_default`. Work remains in `auto` mode. Because that mode still asks before a command, the fixture handles only the exact tool-call ID, `bash` name, and command-only input, then answers the current complete native pending approval binding once. It neither uses `bypassPermissions` nor broadens authorization.

Assertions require the actual next provider request and persisted successful native tool output to contain both the unique marker line and a distinct `Linux` line, excluding a `Darwin` line. They also require exit code 0, no tool timeout, the exact final answer, a successful saved assistant row, and the normal native terminal.

The VM execution boundary is that one default Bash call. Session setup alone is not a VM exercise. The current OCI implementation returns a successful tool result only after its delete / Host shutdown path; this fixture additionally aborts and closes its owned ACP runtimes, waits/reaps their child handles, closes late runtimes, and joins the provider before printing its PASS marker. Assertion and timeout panics pass through the same finally cleanup before rethrow. Failure-side ACP closure also closes the owned OCI Host's stdin; this fixture does not independently enumerate or reap grandchild VM PIDs.

The turn has a 120-second test deadline to allow the current 45-second OCI startup and 30-second Host settlement bounds. The root runner retained a separate whole-process deadline and ownership observation: async child kill/locks and the provider thread join are not covered by a single fixture-wide timeout. The actual run did not time out and left no observed owned process.

## Source checks

Scoped `rustfmt --edition 2024 --check --config skip_children=true` and scoped `git diff --check` both exit 0. The first formatting check requested only alphabetic placement of the new module declaration; that small correction is reflected in the final source, and the first check log is preserved separately. See the check result/log files and `source-before.json` / `source-final.json`.

The fixture author ran no Cargo command, GUI/CUA action, model call, VM or T01 execution, or asset mutation. Compilation and the successful actual guest run above were root-owned. R3 has no new paid CUA result; R2's original assessment and its eventual terminal/oracle evidence remain separate.
