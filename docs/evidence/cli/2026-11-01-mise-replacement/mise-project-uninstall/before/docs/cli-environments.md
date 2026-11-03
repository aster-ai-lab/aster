# CLI environments and project tool locks

The CLI reads tools and variables from ACL. User declarations live at
`$ASTER_HOME/env.acl`; project declarations live at `.aster/env.acl`.
Project discovery walks ancestors from the filesystem root to the current
directory. Nearer declarations override earlier values.

```acl
env {
  tools {
    tool "uv" {
      version = "latest"
    }
  }
  vars {
    var "APP_MODE" {
      value = "development"
    }
  }
}
```

Install and select tools through the current catalog:

```console
aster tool install
aster tool install uv@0.9.5 fd@10.3.0
aster tool use uv@0.9.5 fd@10.3.0
aster tool ls
aster tool ls-remote uv
aster tool which uv
aster exec -- uv --version
```

`tool install` and `tool use` accept multiple requests, retaining their order
and ignoring identical repeats. A bare name in `install` uses the merged
project/user declaration when present. Without a declaration, it requests
`latest`. Explicit installation can include several versions of one tool.

`tool use` validates the complete argument list and installs every selection
before writing the declarations once. It writes resolved concrete versions.
A bare name prefers the newest stable installed version, falling back to an
installed prerelease; explicit `@latest` requests the provider's latest version.
Already verified installations remain if a later acquisition fails, while the
declaration file remains unchanged. Requests for several versions of the same
tool become an ordered version list; identical resolved versions appear once.
The first version supplies its commands first, and later versions can supply
additional publisher-declared commands. `tool which` uses the same order.
`tool use --global` writes the user declaration. Project execution prepends the selected installation
directories to PATH; it does not depend on the store's global current pointer.

ACL accepts either a single version or an ordered list:

```acl
env {
  tools {
    tool "node" {
      version = ["20.11.0", "18.19.0"]
    }
  }
}
```

A nearer declaration replaces the entire list for that tool. Bare `install`
installs every declared version without rewriting the file. `env set` and
`env unset` preserve tool lists. `env show --json` represents one version as a
string and several versions as an array.

Remove one exact installed version with `aster tool uninstall uv@0.9.5`.
Other versions remain installed. Removing the current version clears its
pointer; it does not choose another version. On Unix, project dispatch shims
are rebuilt for the versions that remain installed. Project
declarations remain in place unless `--unpin` is supplied. With an exact
version, `--unpin` removes only the same literal project pin, preserving other
versions of that tool and all environment variables. Invalid declarations refuse
`--unpin` before installation files are removed.
An identified installation can also be removed when its directory or main
program is missing; its valid receipt supplies the removal identity.

`aster tool uninstall uv` requires exactly one matching installed version.
Bare version fragments follow the same rule; an exact stored label takes priority.
Use `--all` to remove every matching version of the named tool. Ambiguous
requests refuse before changing installations or declarations.
Multiple tools can be selected in one command, for example
`aster tool uninstall uv@0.9.5 fd@10.3.0`. Every request is resolved before
removal starts; a later invalid or ambiguous request leaves earlier tools in
place. Repeated requests for the same installation remove it once.

`aster tool uninstall --all` selects every recorded managed installation,
including older versions without a current pointer and installations whose
executable is missing. It does not remove unrelated project declarations unless
`--unpin` is supplied. For this global form, pins without a recorded
installation remain in place. Discovery is bounded and refuses malformed per-version receipts or linked tool
directories before removal. Filesystem failures during removal are reported;
the batch does not promise rollback of earlier successful removals.

Explicit operators such as `prefix:0.9`, subtraction and provider aliases
resolve through source-identified version metadata and select only the resolved
version. If it is not installed, an older installed match is preserved.
Missing cached metadata is refreshed by the CLI through the existing source
adapter; the store's candidate API itself performs no network requests.

`--dry-run` previews selected versions without removing payloads or pins.
`--dry-run-code` also returns status 1 when matches exist, or 0 for no matches.
Both flags can be combined. JSON includes a deduplicated `tools` array of
`{ "name", "version" }` identities and a `dryRun` flag. Single-tool JSON
retains `wouldRemove` for previews and `versions` for removal. With bare fragment selection, `--unpin`
also removes a project pin naming one of the selected concrete versions.
`--project` and complete backend-specific selection remain required lifecycle
work. The full replacement acceptance matrix remains open.

Use `env set NAME=value`, `env unset NAME` and `env show` to edit or inspect
variables. `env show --json` explicitly prints their values. Debug output
redacts values, but explicitly requested environment output is not secret
storage.

## Project dispatch shims (Unix)

`tool install` and `tool use` rebuild the commands under
`$ASTER_HOME/tools/shims`. Add that directory to PATH for commands launched
without `aster exec`. The shim reads the current directory's declarations,
uses their ordered versions and forwards native command arguments. A missing
selected version uses the normal installation path. A command without a
configured owning tool refuses instead of using the global current pointer.

`aster tool reshim` repairs missing entries and includes commands from every
recorded version, including commands supplied only by a later version. It
removes obsolete generated dispatch shims while preserving other entries.
Run it again after moving the Aster executable. The generated shim retains its
tool-store location when the calling environment uses another `ASTER_HOME`.

Windows retains the existing pointer shims. Shell activation still exports a
snapshot; dynamic activation, tool stubs and the remaining synchronization
modes remain required by the full replacement plan.

## Offline and locked operation

```console
aster tool install --offline
aster tool lock --offline
aster tool install --locked --offline
aster exec --locked --offline -- uv --version
```

Offline mode never downloads. Missing installations return an error before
the requested command starts. Without a lock, version requests select matching
identified installations; `latest` uses available provider aliases and prefers
stable installed versions, falling back to an installed prerelease when no
stable version exists. It does not follow the store's global current pointer.

Tool declarations accept exact and partial versions, explicit `prefix:`
requests, and subtraction such as `sub-1:latest`. A bare request prefers an
existing exact version; `prefix:` selects the newest matching version. Matching
uses version separators, so `0.8` does not select `0.81.0`. Backend aliases such
as npm dist-tags are resolved from provider metadata.

`tool ls-remote` refreshes a bounded provider listing and its aliases.
`tool ls-remote --offline` reads the cached listing. Installed prefix requests
can resolve without remote metadata; aliases and subtraction from `latest`
need cached metadata. Publisher prerelease flags are retained, and explicitly
named prereleases remain selectable. Version cache data is separate from
project declarations and locks.

Native `path:` declarations use an existing runtime directory. Relative paths
resolve from the declaring project directory, including ancestor declarations;
relative user declarations resolve from the native user home. `system` uses
the inherited executable search path with Aster's managed shim/install
directories excluded. Neither mode downloads or creates managed receipts.
Node, npm, pipx and GitHub default layouts are supported; layouts that require
Aqua registry metadata still refuse.

Use `aster exec -t node@path:./sdk -- node --version` to select a runtime for
one execution. Repeat `-t` to select each tool's versions in argument order;
identical requests collapse.
Other declarations retain their original roots. Overrides do not write ACL
files and cannot be combined with `--locked`.

Provider metadata and archive responses are capped at 200 MiB. Download errors
retain their underlying cause while omitting the request URL from diagnostics.

The catalog includes the official prebuilt Node runtime (`core:node`). Node
installation verifies the release's detached GPG signature and archive checksum,
retains the complete native layout, and exposes its bundled npm and any npx or
corepack commands present in that version. Full SemVer pins reuse identified
installed versions without fetching version metadata or distribution bytes;
latest, prefixes and LTS aliases still resolve from provider metadata. This does
not implement Node source builds, flavors or the complete runtime option set.

`tool lock` writes `.aster/env.lock.acl` for the current OS, architecture and
ABI. It binds all effective tool declarations to exact versions and the
SHA-256 of their source-declared files. The lock excludes environment variable
values and machine-local absolute paths. Commit the generated lock with the
project; regenerate it with `tool lock` after changing tool declarations.
The v3 lock retains the resolved selector and original request from v2, and adds
archive checksums plus a digest of the complete native layout record. File
hashes/modes, link targets, command paths and PATH directories are bound by that
record. V1/v2 file locks remain readable for supported request kinds. Locked execution
uses the sealed installations and continues to work when remote metadata cache
entries are absent.
The v4 lock records multiple versions of the same tool in declaration order,
retaining each request and its complete verified installation. Reordering or
changing the list invalidates the lock. Single-version locks retain the earlier
schemas and their compatibility.

Locked commands find the nearest ancestor lock and refuse a missing lock,
declaration drift, a different platform, missing companions, changed bytes or
unexpected files. Online locked installation can fetch an absent version, but
checks the complete downloaded file set before publishing it. Online locked
downloads still query the provider to discover the archive URL; pre-resolved
download locations are not yet recorded in the lock. `--no-install`
on `exec` also prevents downloads when combined with `--locked`.

Lock generation checks the installer's recorded membership against disk. An
older installation without a file manifest must be reinstalled before it can
be locked. Lock reads are limited to 1 MiB; verification accepts up to 8192
entries per installation and hashes at most 4 GiB across an operation.

## Current limits

This is a partial mise replacement. Reference execution, path/system locks,
user-defined aliases, multi-version shims/stubs/synchronization, core runtime
layouts, the complete backend/plugin catalog, tasks and host-state
management remain under implementation. The Node prebuilt layout is implemented;
the other required core providers remain open. A lock currently describes one
platform. Shell activation emits a snapshot; directory hooks and restoration
are not implemented.
Windows and Linux native acceptance remains unverified.

The full required capability inventory and validation evidence are tracked in
[`specs/031-cli-mise-replacement`](../specs/031-cli-mise-replacement/spec.md) and
the [dated outcome](evidence/cli/2026-11-01-mise-replacement/OUTCOME.md).
