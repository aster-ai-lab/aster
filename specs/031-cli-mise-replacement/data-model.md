# Environment, tool, and task contracts

This is the implementation target for the remaining tasks, not a claim that
all these types already exist. Keep existing public Rust APIs; introduce typed
values at new admission boundaries and pass them inward without revalidation.

## Tools and versions

- **Tool identity**: an exact registry key or a parsed backend identifier.
  Registry aliases come from stored registry data, never inferred names.
- **Version request**: exact/bare label, explicit prefix, latest, declared alias,
  source reference, existing path, system selection, or subtraction request.
  Tool-specific aliases and supported reference kinds are resolved by their
  declared backend. Alias cycles and unsupported combinations refuse.
- **Resolved distribution**: concrete identity/version, target OS/architecture/
  ABI, original source identity, download location, integrity/verification
  requirements, archive layout, entrypoints, and required companion files.
- **Installation**: immutable version contents and per-version identity record.
  `current` remains a compatibility convenience, not authority for an exact
  project selection. Legacy current receipts are accepted only for that exact
  version; an unidentifiable older directory must be reinstalled.
- **Project lock**: request-to-distribution mapping with platform and integrity
  data. It is separate from machine-local installation receipts. A locked run
  checks both declaration drift and artifact identity before starting a command.

The store has per-version records at `receipts/<tool>/<version>.json`; their
wire format remains the existing receipt schema. New installations also write
`<version>.files.json` from the executable and companion membership admitted
by the source. Legacy installs without this metadata cannot create a lock by
inferring membership from whatever remains on disk.

`.aster/env.lock.acl` binds the current platform (OS, architecture and ABI),
the effective tool requests, exact distribution identities, and SHA-256 of
every source-declared installed file. Lock generation checks the expected
manifest against disk; locked acquisition checks downloaded membership and
bytes before publication. It never stores environment variable values or
machine-specific absolute paths. Reads are bounded to 1 MiB; file inspection
is bounded to 8192 entries per installation and 4 GiB per operation. Multiple
platform records, native dependency sidecars, backend resolution and the
remaining upstream lock behaviors are still required for FR-004.

V1 requests have a distinct legacy selector kind: every admitted version label
is literal except the original `latest` sentinel. These locks retain v1 wire
semantics on rendering. V2/v3 selectors hold validated current requests. Catalog
identity resolution does not reinterpret a locked v1 label through the newer
grammar. Bounded ACL admission retains legacy literal labels so the versioned
lock can resolve them; it does not guess an unlocked declaration's meaning.

The implemented `VersionRequest` has a private admitted kind and bounded syntax.
`VersionListing` holds at most 10000 versions and aliases, sorts with the frozen
upstream's `versions 8.0.1`, and resolves aliases and subtraction within 32
steps. A bare query prefers an exact version; an explicit prefix selects the
last match. Provider prerelease metadata is preserved. Reference execution still
refuses before fetch. Path/system selection uses a separate admitted origin;
actual CLI comparisons against seven context references pass.

`LoadedEnvConfig` retains the logical root of each merged tool declaration.
Project paths use the declaring workspace; user declarations use the native
user home rather than ASTER_HOME. Argument-relative paths use cwd. A
`ResolvedEnvTool` holds either a managed selection, a system selection, or a
canonical existing external directory with native executable search paths.
External/system selections create no receipts or current pointers and need no
HTTP client. Native lookup reuses platform executable/PATHEXT handling; system
selection excludes emitted Aster shim/install directories from inherited PATH.
External GitHub discovery is bounded to the existing 8192-entry budget. Aqua
layouts needing registry metadata refuse explicitly. Temporary overrides replace
declarations before inspecting external paths, resolve relative to cwd and retain
other tools' declaration roots. They do not write ACL. The CLI exposes repeated
`--tool` flags, with the final request for a tool winning; locked execution
refuses overrides. Library, pager and actual CLI tests and the paired build pass.
Path/system lock capture, full backend layouts and native platform acceptance remain
required; this model does not imply those contracts are complete.

Machine-local provider metadata lives under `cache/versions/<spec-digest>.json`,
separate from project configuration and receipts. Records bind their schema and
exact source spec; reads and writes are bounded to 8 MiB. Offline resolution
does not create an HTTP client. Installed selection uses per-version identities
and provider metadata rather than the global current pointer. Subtraction from
`latest` refuses without cached metadata instead of guessing an earlier base.

The HTTP entry boundary caps decoded response bodies at 200 MiB for metadata
and archives, including bodies without a declared length. Policy refusals stay
distinct from typed HTTP, body-read and JSON parser failures. HTTP errors have
their request URL removed before entering the public error chain; synthetic
query secrets and response payloads are absent from Debug and Display tests.

Lock v3 retains the original request and the resolved selector from v2, allowing
aliases and subtraction to remain fixed. The reader accepts v1/v2 file locks for
supported request kinds; legacy literal labels colliding with new operators
still need migration behavior. After lock verification, exec builds PATH from
the selected installation records directly;
it does not re-resolve them through a mutable or missing metadata cache.
Pre-resolved artifact URLs and the upstream no-registry-lookup locked-download
contract remain unimplemented; current online locked fetches still discover
URLs through the owning source API before checking their expanded contents.

Native archive sources may return `InstallPayload::Archive` through the default-
compatible `ToolSource::fetch_installation` protocol. `ArchivePayload` admits
publisher SHA-256, exact prefix, command paths and PATH directories. Extraction
retains complete tar.gz/ZIP contents and internal native links; file data goes
to disk rather than a second fully expanded in-memory payload. Input is bounded
to 200 MiB compressed, 8192 entries and 4 GiB expanded. Tar extension headers
are preflighted before the library buffers them. A real Node v22.21.1 macOS
arm64 archive was observed to contain 6195 entries, 5031 regular files and three
links; this count is layout research, not installation or GPG acceptance.

Per-version `<version>.archive.json` binds source identity, archive checksum,
entrypoints, PATH directories and typed file/directory/link membership. Native
file modes are retained on Unix. `InstallationPaths` admits receipt identity
and resolves entries within the installation; CLI execution does not flatten
the layout. Legacy standalone receipts and payload APIs remain usable. V3 ACL
archive locks contain the compressed archive SHA-256 and the SHA-256 of the
deterministic v1 archive record serialization. That record binds every member,
file hash/mode, link target, entrypoint and PATH directory. Capture and offline
reuse verify the record against disk; absent locked installations validate both
checksums before publication. The compact lock avoids duplicating thousands of
member records in project configuration. Complete directory metadata,
pre-resolved download locations and backend-specific budgets remain required.

The official AIX source uses the existing signed plugin catalog and TUF trust
model. A package target whose suffix is
`oci-ghcr-v1/<domain>/<manifest-sha256>/<name>.aix` selects the exact OCI manifest
under `ghcr.io/aster-ai-lab/plugins/<domain>/<publisher>/<name>`. The domain is
explicit signed category data. The existing catalog/lock target name seals the
manifest identity; archive SHA-256 and length seal its one native AIX blob.
No public catalog fields or frozen fixture digests are rewritten. The existing
cache transaction verifies and resumes blob bytes; expired/untrusted metadata
and changed contents refuse. Root-key establishment and real publication remain
required. Local cached TUF metadata now decodes file URLs through native paths
instead of treating percent-escaped text as a filesystem path.

Native AIX publication preparation is an immutable admitted value. It binds
explicit domain/publisher policy, declared reviewer identifiers, the native
`plugins/<domain>/<publisher>/<name>` source path, included LICENSE, expanded
package inventory, compressed archive identity and exact OCI manifest bytes.
The archive must match the current complete source fingerprint. This value
does not grant GitHub authority, sign metadata or establish a trusted root.

The `aster use` composition root reuses the local SDK's parser and fallible
output writer. Standalone `aster-use` keeps its public parser API and wire
envelope. Native arguments cross one UTF-8 admission boundary for SDK parsing;
invalid-byte filesystem operands remain unsupported. An explicit
`ASTER_USE_BIN` retains the existing native subprocess contract. SDK tasks run
on workers with the standalone command's 8 MiB stack requirement. Environment
commands default to one async worker and prewarm one blocking worker under the
existing pool capacity; explicit worker configuration remains authoritative.

## Configuration and environment

Exact version removal uses the existing store lock, validated installation
identity and owned native filesystem adapters. It deletes that version's tree
and receipt/layout records while retaining other versions. If it removes the
current version, the matching current receipt, pointer and declared command
shims are cleared without choosing a fallback. Receipt admission is shared
with execution, while removal does not require the payload files to remain
intact. Execution retains its complete-installation check. Declaration editing is a CLI
operation: optional `--unpin` first admits the existing ACL and removes only
the same literal version pin, or the named pin for the legacy all-version form.
The read-only candidate selector gives exact stored labels priority over
bare fragments and admits archive metadata before returning the selected records.
Explicit operators and provider aliases resolve through source-identified
cached listings, selecting only the resolved installed version. The CLI may
refresh a missing listing through the existing HTTP source. A missing resolved
version does not fall back to an older installed match.
Ordinary executable selection still checks completeness. A single CLI request
must have at most one candidate unless `--all` is explicit. Preview returns
`wouldRemove` without deleting payloads or editing ACL; `--dry-run-code` returns
status 1 for a nonempty selection. Multiple-tool/project/global selection and
late-I/O rollback remain open.

- A declaration layer has its ACL path, source identity, scope, and admitted
  fields. Discovery applies user then root-to-nearest project declarations.
- An evaluated environment distinguishes set, unset, required, default,
  source-file, PATH, virtualenv, hook, and alias operations. Evaluation preserves
  the order/provenance needed for deterministic precedence and restoration.
- Shell activation retains the original values of changed variables and PATH
  entries, recomputes the active project on the shell's supported hooks, and
  restores values when declarations stop applying.
- External ecosystem metadata and directive inputs are admitted through their
  declared ACL directive. They do not introduce another Aster configuration
  language or widen workspace permissions.
- Secret-bearing values and command arguments have redacted Debug output,
  including containing configuration and command types. Explicitly requested
  environment output is a separate observable operation.

## Tasks and processes

- A task definition has an exact declared name, owner project, command/script,
  argument contract, environment, tool selection, dependencies/order, and
  declared inputs/outputs. Names and aliases come from declarations.
- The admitted graph is bounded and acyclic. Shared dependencies have one
  invocation identity. Scheduling respects dependency and ordering semantics,
  concurrency bounds, cancellation, and task-specific project contexts.
- Process values stay `OsString`/`PathBuf`. Unix explicit `exec` replaces the CLI
  process; Windows uses the existing process-tree job adapter while retaining
  the console group. Task processes use the same platform ownership adapters.
- Cancellation of an active blocking download is not solved by dropping its
  join handle. T004/T008 must supply a real request/work cancellation contract.

## Caches and host state

- Task cache records bind command/arguments, inputs, relevant configuration,
  tools, platform, and outputs. Restored output trees must satisfy the owning
  filesystem/archive contract. Credentials and plaintext secrets stay out of
  persisted keys, metadata, logs, and Debug output.
- Provisioning plans distinguish desired/observed state and the exact operations
  to apply. An unsupported native operation is a refusal, not a successful no-op.
- Dotfile generations retain restoration/history evidence. Daemon identities
  bind a declared service, project context, lifecycle, readiness, and cleanup.

## Native AIX authoring and dependency contributions

- A source package has one validated manifest and a bounded, link-free inventory
  of regular files. The native AIX builder and expanded-package fingerprint share
  that inventory; the existing fingerprint algorithm and published identities
  remain unchanged. Archive order, ZIP timestamps and compression are fixed.
- A built archive remains staged until native inspection reproduces the source
  identity, surface inventory and expanded-file fingerprint. It is published
  outside the source without replacing an existing path. Building grants no
  installation or publisher trust; domain/OCI preparation and signed publication
  remain separate required steps.
- External dependency declarations now have a typed schema-v1 ACL parser in
  the tool acquisition owner. A proposed AIX host adapter uses the existing
  generic Plugin contribution to carry that entry. CLI binary acquisition is
  distinct from the existing non-interactive agent Tool Task contract. The tool
  acquisition owner must admit explicit source/version/platform, immutable
  artifact URL/length/digest and installation layout before a host applies it;
  an external URL must never masquerade as a package-relative executable.
- Required desktop runtime components retain host-owned package-ID/component
  bindings and release membership checks. A plugin label or a recipe's claimed
  `required` flag grants no system-component authority. These acquisition/host
  adapters and the 14 real dependency artifacts remain unimplemented requirements.

## Verification references

The baseline source is pinned by `contracts/mise-baseline.json`. For version and
lock behavior, also review upstream `src/toolset/tool_request.rs` and
`src/lockfile.rs` at the same release commit before implementing T004. Each
state transition requires a behavioral witness, not only serialization tests.
