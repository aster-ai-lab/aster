# Feature Specification: Aster CLI replaces mise

**Feature Branch**: current checkout (no new branch requested)

**Created**: 2026-11-01

**Status**: Implementing; full replacement is not yet achieved

**Input**: User description: "实现aster cli 完全替代 mise"

**Gate context**: `apps/aster`, its existing pager CLI, and
`crates/use/tool-acquire` own this feature. This user-requested capability
extends the CLI without adding another application or workspace.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reproduce a project's tools (Priority: P1)

A developer declares tools in ACL, installs them, and runs commands with the
selected versions. Two projects can select different installed versions.

**Why this priority**: environments and tasks depend on reliable tool selection.

**Independent Test**: the owning Rust suites install fixture payloads, execute
them through the real store and CLI, and assert versions, checksums, arguments,
failure recovery, project isolation, and locked/offline behavior.

**Acceptance Scenarios**:

1. Given different version declarations in two projects, commands in each
   project use its declared version regardless of the last installed version.
2. Given a checksum mismatch, installation fails without changing the previous
   executable, receipts, or project declaration.
3. Given an exact locked installation already present, offline execution
   succeeds without contacting a registry. A missing artifact is refused.

### User Story 2 - Activate and restore project environments (Priority: P1)

A developer changes directories and receives the correct tools, variables,
aliases, and hooks. Leaving the project restores the prior environment.

**Why this priority**: one-time exports do not replace mise activation.

**Independent Test**: real supported shells enter sibling and nested temporary
projects with spaces and non-ASCII paths. Tests assert restoration, quoting,
precedence, unset behavior, and refusal of unsupported shells or untrusted code.

**Acceptance Scenarios**:

1. Given user, ancestor, and nearest project declarations, the nearest value
   wins while unrelated ancestor values remain available.
2. Given a project that changes PATH and variables, leaving it restores the
   original values without accumulating PATH entries.
3. Given an unsupported directive or untrusted executable configuration, the
   operation refuses it with a typed diagnostic before execution.

### User Story 3 - Run reproducible project tasks (Priority: P1)

Developers and CI run named ACL or file tasks with typed arguments, dependency
ordering, parallelism, change detection, caches, and monorepo addressing.

**Why this priority**: an agent's task-planning tool is not a build-task runner.

**Independent Test**: fixture commands produce real files and invocation logs.
Tests assert dependency ordering, concurrency bounds, exit propagation,
cancellation, cache restoration, watcher reruns, and project-specific contexts.

**Acceptance Scenarios**:

1. Given a dependency graph, shared dependencies execute once, independent
   prerequisites can run concurrently, and cycles refuse before side effects.
2. Given unchanged inputs, a task is skipped or its cached outputs are restored
   according to the selected cache mode. Changed inputs invalidate the result.

### User Story 4 - Provision and operate the development environment (Priority: P2)

A developer manages host packages, machine modules, services, dotfiles, remote
hosts, and development daemons through Aster with the baseline mise behavior.

**Why this priority**: these are part of the full requested replacement.

**Independent Test**: plan/status fixtures are hermetic; state-changing adapters
also require native isolated execution on the affected operating systems.

**Acceptance Scenarios**:

1. Given matching declared host state, a repeated apply is a no-op.
2. Given unsupported host operations or missing prerequisites, planning and
   execution explicitly refuse them without reporting success.

### Edge Cases

- Downloads, archives, configuration bytes, process output, and graph work are
  bounded at their entry boundaries.
- Cancellation terminates owned process trees through existing platform adapters.
- Malformed declarations, unknown fields, unsupported backends, incompatible
  locks, missing companions, and unverifiable artifacts fail closed.
- Environment values and provider credentials do not enter Debug or diagnostics.
- Existing Aster command names and its ACP pair remain usable.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Aster MUST install, list, locate, upgrade, uninstall, prune, and
  execute baseline tools and core language runtimes without a mise executable.
  Unsupported platforms, unknown identifiers, and invalid artifacts MUST refuse.
- **FR-002**: Version requests MUST cover exact versions, prefixes, aliases,
  latest, references, paths, system versions, subtraction, and multiple versions
  where the baseline supports them. Invalid or unsupported combinations refuse.
- **FR-003**: Each execution MUST select versions from its project context,
  without using another project's mutable current pointer as authority.
- **FR-004**: Project locks MUST retain resolved versions, sources, platform
  identities, and verification data. Locked and offline modes MUST refuse drift
  or unavailable artifacts rather than silently resolve another version.
- **FR-005**: ACL environments MUST support layered discovery, named environment
  profiles, variable unset/default/required behavior, dotenv/source directives,
  PATH additions, templates, virtual environments, aliases, hooks, and the
  baseline secret integrations. Unknown directives MUST refuse.
- **FR-006**: Shell activation, deactivation, shims, and explicit execution MUST
  preserve the baseline selection and restoration behavior on supported shells.
  Unknown shells MUST not receive guessed Bash syntax.
- **FR-007**: The task runner MUST cover named and file tasks, argument parsing,
  templates, ordering, dependency graphs, parallelism, output modes, watch,
  incremental checks, local and remote caches, and monorepo project graphs.
  Invalid graphs, arguments, cache records, and untrusted scripts MUST refuse.
- **FR-008**: Tool, backend, environment, and package-manager extension contracts
  MUST cover the baseline ecosystem, including legacy compatibility where the
  frozen baseline supports it. A parsed backend name alone is not coverage.
- **FR-009**: Bootstrap, host packages, accounts, files, services, firewall,
  Compose, Git repositories, remote hosts, dotfiles, history, synchronization,
  and development daemons MUST have baseline-equivalent behavior and refusals.
- **FR-010**: Diagnostics, completion, tool documentation/skills, verification
  policies, and MCP access MUST expose the implemented environment capabilities
  without changing Aster's existing agent and package-manager contracts.
- **FR-011**: The acceptance inventory MUST retain all features of the frozen
  mise baseline, including experimental features and their status. Each item
  MUST link implementation and behavioral evidence before completion.
- **FR-012**: Functional effectiveness and efficiency MUST be demonstrated on
  representative fixed workloads. Installed immutable runtimes MUST not be
  downloaded again merely to select them; offline paths MUST make no network
  requests; cache hits MUST avoid rerunning task effects. Record cold/warm
  latency, resource use and request/process counts with OS, toolchain, features
  and build profile. Profile measured hot paths before changing their ownership
  or cache layout; debug timings are not release-performance acceptance.
- **FR-013**: The official trusted plugin source MUST use the Aster organization
  repository `aster-ai-lab/plugins`, organize native AIX plugins by explicitly
  declared domains, and distribute `.aix` artifacts through associated GitHub
  Packages/GHCR packages. Signed catalog evidence MUST bind immutable OCI and
  AIX identities. Native package checks, installation/update/offline behavior
  and publication evidence are required; repository ownership alone is not
  sufficient trust evidence.
- **FR-014**: Aster Desktop MUST expose Plugins in its ActivityBar without an
  account, remove Environment Dependencies from local settings, and redirect
  legacy dependency links to Plugins. Plugin discovery MUST read the official
  `aster-ai-lab/plugins` Registry through the native verifier. Preserve install,
  update, removal, status and progress workflows, including required runtime
  component protection. Convert all eleven preset dependencies and three runtime
  components into domain-organized native AIX packages. External native tool
  artifacts, including CUA's CLI releases, require explicit immutable URL,
  platform, size, digest and installation-layout declarations; they must not be
  executed through unverified installer scripts or inferred release filenames.

### Key Entities

- **Environment declaration**: ACL owned by `aster-use-tool-acquire`.
- **Version request / resolved tool / project lock**: tool-owner typed data.
- **Task definition / invocation / result / cache record**: task-owner data.
- **Provisioning plan / daemon state / dotfile generation**: owning module data.
- **Baseline feature witness**: source reference and acceptance evidence in this
  spec's contracts and `docs/evidence/cli/2026-11-01-mise-replacement/`.

## House Checklist *(mandatory — from Agents.md)*

- [ ] New behavior has a small hermetic test in the owning package (T001–T011).
- [ ] Closed sets have explicit parsing and formatting (T002–T011).
- [ ] Production fallible paths do not panic (all implementation tasks).
- [ ] Internal Aster dependencies remain local (T012).
- [ ] Product configuration is ACL parsed by `aster-acl` (T002–T011).
- [ ] Evidence target is `docs/evidence/cli/2026-11-01-mise-replacement/OUTCOME.md`
      (T013).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every feature in the frozen upstream inventory has an Aster
  implementation and passing behavioral witness; no required item is deferred.
- **SC-002**: Owning-package tests, Clippy, format, dependency checks, and native
  platform acceptance pass with the pinned toolchain and recorded features.
- **SC-003**: The final CLI and ACP pair execute the acceptance scenarios with
  mise absent from PATH and without substituting a mise binary or service.
- **SC-004**: Fixed release-profile cold/warm workloads have retained latency
  distributions and resource/request counts. Confirmed redundant I/O, downloads
  and task execution are eliminated, and material scaling regressions are
  resolved before claiming the replacement is efficient.

## Assumptions

- The comparison baseline is mise `v2026.10.25`, its source, and the official
  feature inventory captured on 2026-11-01. Later upstream additions are not a
  moving completion target.
- Functional equivalence uses Aster command namespaces and ACL configuration.
  Existing `aster use` and `aster mcp` meanings are preserved.
- Existing platform tools used by upstream backends remain legitimate runtime
  prerequisites; none may be silently assumed or downloaded without policy.

## Explicitly Not Claimed

- No completion is claimed from plans, help text, parsed backend names, or tests
  that bypass the real implementation.
- Byte-compatible mise configuration syntax conflicts with the repository's
  ACL-only product contract. Equivalent configuration behavior uses ACL.
- No publication, release certification, or Git commit is requested here.
