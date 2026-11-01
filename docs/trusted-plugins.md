# Aster trusted plugin source

The official authoring repository is
[`aster-ai-lab/plugins`](https://github.com/aster-ai-lab/plugins). Installable
`.aix` packages use OCI artifacts in GitHub Packages (GHCR) associated with
that repository. The three macOS ARM64 runtime components have published catalog
records; the remaining tool and application plugins still need publication.
This source does not depend on mise's
registry or executable.

The package host builds the existing `aster-use` CLI and runs its native
`registry package build`, `check` and `prepare` commands. The desktop installs
through the same Rust SDK in process. Final release archive and remote GHCR
installation acceptance remain required.

## Local source checkout

The official repository is checked out at `plugins/` as a public Git submodule.
From the Aster repository root, initialize the exact recorded revision with:

```sh
git submodule update --init plugins
```

The submodule tracks `main`, while the parent gitlink pins a reviewed commit.
Updating the source requires committing the new gitlink in Aster. Plugin sources
belong to the submodule; Aster does not track them individually or add another
Cargo workspace. `target/plugins-bootstrap/source` is temporary build staging,
not the maintained source checkout. Native archives and models remain in GHCR.
The authoritative directory and maintenance rules are in the submodule's
[`docs/layout.md`](https://github.com/aster-ai-lab/plugins/blob/main/docs/layout.md).

## Package organization

Use one source directory per plugin. The primary domain is explicit repository
metadata and must agree with the signed catalog's categories. Do not infer a
domain from names, descriptions or file contents. Additional categories are
search metadata; they do not duplicate a plugin's source or identity.

```text
plugins/                         # repository root
├── README.md
├── CONTRIBUTING.md
├── domains.acl                  # admitted domain IDs and ownership
├── plugins/
│   └── <publisher>/
│       └── <name>/
│           ├── aster-use-extension.acl
│           ├── README.md
│           ├── LICENSE
│           ├── skills/          # only surfaces declared by this plugin
│           ├── tools/
│           └── tests/
├── registry/
│   ├── metadata/                # signed TUF metadata, public keys only
│   ├── catalog/<publisher>/<name>/<version>/<channel>/<target>.json
│   ├── locks/runtime/<target>.json
│   └── root.sha256
├── recipes/runtime/<target>.py
├── scripts/
├── tests/
├── docs/
└── .github/                     # CODEOWNERS and workflows
```

The native preparation command admits this bounded ACL policy shape (the
identifiers below are illustrative, not an installed production policy):

```acl
plugin_domains {
  schema_version = 1
  domain "development" {
    owners = ["example-maintainer"]
    publishers = ["example"]
  }
}
```

Each domain must be unique and declare nonempty, unique owner and publisher
lists. Unknown fields, unknown domains, unadmitted publishers, invalid native
identifiers, unsupported schemas and oversized/non-UTF-8 policy files refuse
preparation. Owner identifiers describe review responsibility; they do not
grant GitHub permissions or replace signature verification. Publication
automation must still enforce actual reviewer and workflow authority.

An initial domain vocabulary can cover `development`, `operations`, `data`,
`knowledge` and `productivity`. These are author-declared categories, not an
automatic classifier. A new domain needs an explicit domain/ownership entry.
The package ID remains `<publisher>/<name>` when a domain changes. Existing
digest-pinned releases must remain available at their original addresses.

For example, source under `plugins/aster/<name>/` publishes to:

```text
ghcr.io/aster-ai-lab/plugins/aster/<name>:<target>-<archive-sha256>
ghcr.io/aster-ai-lab/plugins/aster/<name>@sha256:<manifest-digest>
```

Tags support discovery. Installation and project locks use immutable digests.
Version, channel and platform are separate catalog fields. The logical TUF
package target retains the existing package/version/channel/target prefix and
adds the versioned OCI location contract:

```text
extensions/<publisher>/<name>/<version>/<channel>/<target>/
  oci-ghcr-v1/<manifest-sha256>/<name>.aix
```

This is one continuous target name. Domains are explicit signed categories and
publication admission metadata, independent of the package storage identity.
Its SHA-256 is exactly 64 lowercase hexadecimal digits.
Existing catalog digests and package locks therefore retain the OCI manifest
identity together with the AIX blob digest, without adding mutable tag lookup.
Ordinary HTTP targets retain their existing names and behavior.

## AIX and OCI contracts

The OCI reader requires one native `.aix` ZIP payload. Its artifact
type is `application/vnd.aster.aix.v1` and payload media type is
`application/vnd.aster.aix.v1+zip`. OCI is the distribution envelope; the AIX
payload retains its root `aster-use-extension.acl` schema-v3 manifest, README
and complete declared surface files. The official publication policy also
requires a license, declared package identity/version and supported target.

The OCI manifest digest and the `.aix` blob SHA-256 identify different bytes.
Signed publication metadata must bind both, along with blob length and native
package/manifest evidence. Native AIX validation runs before publication and
again before installation. Installation retains the existing workspace grants
and surface authorization checks.

The reader admits an OCI image manifest with the AIX artifact type, the empty
OCI config descriptor, one correctly named AIX layer and the exact repository
source annotation. Token responses are bounded to 64 KiB; manifests to 1 MiB,
including responses without Content-Length. Blob verification and resume use
the existing signed-length/SHA-256 cache transaction. Authorization is sent only
to the initial GHCR origin when following storage redirects. Cached or offline
reuse performs no GHCR request.

`inspect_aix_package` in `aster-use-extension` reuses the installer archive
reader, manifest/inventory admission and native surface-file checks. It hashes
the complete expanded package without installing or executing it. The CLI entry
is `aster use registry package check <file.aix> --json`; standalone SDK users can
invoke the same command as `aster-use registry package check`.
This format check alone does not establish publisher identity or signatures.

Native UI package inventory includes the complete declared root, including
license notices and other non-web files, in its bounded content identity.
Browser projection exposes only supported web media; it does not serve those
other files. The strict `list_ui_package_assets` API retains its existing refusal
of unknown media. All 35 existing `apps/use-registry/packages` sources now build
as native AIX archives, including the five previously refused for bundled legal
notices. Their missing per-package licenses and publication metadata still need
review before admission to the official source.

## External tool archives

The tool owner now parses bounded schema-v1 `toolchain` ACL through
`aster_use_tool_acquire::ToolchainPlugin`. It downloads through the existing
GitHub release transport, or accepts already downloaded archive bytes without
constructing an HTTP client. Both paths check the exact host platform, declared
length, SHA-256 and complete source-declared archive layout before publication.
The declaration is limited to 64 KiB and downloads to its declared length,
within the existing 200 MiB limit. Unknown fields, credentials, query strings,
unsupported platforms and archive encodings refuse. No upstream installer runs.

For example, CUA CLI 0.9.0 on macOS arm64 has this acquisition declaration:

```acl
toolchain {
  schema_version = 1
  name = "cua"
  spec = "github:trycua/cua"
  version = "0.9.0"
  executable = "cua"
  platform = "aarch64-macos-"
  url = "https://github.com/trycua/cua/releases/download/cua-sdk-v0.9.0/cua-cli-0.9.0-darwin-arm64.tar.gz"
  length = 73285759
  sha256 = "8d15f4b82694ee8e1c78b419331795bfb3f5fcbab80a7d03ab409a9da89c0bae"
  format = "tar.gz"
  prefix = ""
  bin_dirs = [""]
  executable_mode = "0755"
  command "cua" { path = "cua" }
  command "cua-vz" { path = "cua-vz" optional = true }
  command "cua-microvm-init" { path = "cua-microvm-init" optional = true }
  command "cua-spacesd-linux-aarch64" { path = "cua-spacesd-linux-aarch64" optional = true }
  command "cua-spacesd-linux-x86_64" { path = "cua-spacesd-linux-x86_64" optional = true }
}
```

The platform spelling follows the existing tool lock's architecture/OS/ABI
identity. It distinguishes GNU and musl Linux. The archive format is explicit
(`tar.gz` or `zip`); no filename heuristic selects a format or executable.
`executable_mode` defaults to `archive`. The explicit `0755` value applies only
to present, declared regular command members. Other file modes and the complete
archive membership remain intact; the installed record includes the resulting
modes. Windows retains its native executable semantics.

The pinned producer manifest and downloaded macOS archive agree on all bytes.
The archive contains four files, including three companions. The producer's
installer explicitly makes present helpers executable; it also coordinates an
existing daemon during updates. An isolated native acquisition test retained
all four exact file lengths, applied those modes and successfully ran
`cua --version` for 0.9.0. That daemon behavior, publisher-signature
verification, other native platforms and final desktop integration remain
required. The source evidence is in
`docs/evidence/cli/2026-11-01-mise-replacement/cua-cli-external-source.json`.

An AIX generic `plugin` contribution can carry this ACL as its declared entry.
The desktop still needs the typed acquisition adapter and lifecycle ownership
before such a package can be advertised as an installed tool. Merely projecting
the entry file is insufficient. The acquisition library does not itself grant
publisher trust, execute the tool, publish a Registry or produce the 14 plugins.

## Desktop dependency migration

Aster's ActivityBar exposes Plugins without an account. The local settings menu
omits Environment Dependencies; both former dependency routes redirect to
`/app/plugins`. ToYou retains its managed navigation and catalogue behavior.

The native `/api/plugins` adapter resolves an explicitly configured, enabled,
caller-pinned Registry whose URL is exactly the main-branch `registry/` tree in
`aster-ai-lab/plugins`, or the public root pin shipped with the desktop. Missing
configuration returns an explicit setup state; ambiguous or invalid configuration
refuses. Installation still requires a trusted source. The adapter never
substitutes ModelScope or the old hardcoded dependency list. Pagination,
search, category counts and installed filtering operate on verified compatible
stable records. The page has refresh/retry, installation status and progress,
updates and removal. These are local source changes, not a published desktop
release. A separate live SDK check now verifies the three official runtime records
against the root pinned in the repository's `ASTER_PLUGIN_ROOT_SHA256` variable.
That online query does not establish anonymous artifact access or complete plugin
management acceptance.

Install requests bind package ID, selected version and archive digest. Downloads
reuse the TUF/GHCR verifier and `ExtensionLifecyclePackage::prepare_remote`, then
the existing desktop AIX projection. An archive must use `.aix` and satisfy the
host's existing compressed and expanded limits. Unsupported Tool/Flow surfaces
are unavailable until their owning host adapter is implemented. Sandbox and
Document cannot be removed; registry labels do not grant system-component
authority.

The migration inventory below preserves the order and names in
`apps/studio/src/shared/data/presets/binaryTools.ts`, followed by the cards in
`RuntimeComponentsSection.tsx`. Native component requirements remain owned by
`apps/studio/src-tauri/src/runtime_components.rs`.

| Existing item | Package ID | Authoring domain | Existing acquisition contract |
| --- | --- | --- | --- |
| Aster Box (`aster-box`) | `aster/aster-box` | operations | `aster-box` |
| CUA (`cua`) | `aster/cua` | operations | `trycua/cua` CLI release manifest |
| uv (`uv`) | `aster/uv` | development | `uv` |
| Bun (`bun`) | `aster/bun` | development | `bun` |
| fd (`fd`) | `aster/fd` | development | `fd` |
| ripgrep (`rg`) | `aster/rg` | development | `rg` |
| RTK (`rtk`) | `aster/rtk` | development | `rtk` |
| Lark CLI (`lark-cli`) | `aster/lark-cli` | productivity | `github:larksuite/cli` |
| GitHub CLI (`gh`) | `aster/gh` | development | `gh` |
| Notion CLI (`ntn`) | `aster/ntn` | productivity | `npm:ntn` |
| BabelDOC Stream (`babeldoc-stream`) | `aster/babeldoc-stream` | knowledge | `pipx:babeldoc-stream`, minimum `0.6.4.post4` |
| Aster Sandbox (`aster-sandbox`) | `aster/aster-sandbox` | operations | Required bundled sandbox component |
| Aster Document (`aster-document`) | `aster/aster-document` | operations | Required bundled document component |
| Aster Office (`aster-office`) | `aster/aster-office` | operations | Optional bundled Office component |

This is the source-owned conversion inventory, not a claim that these 14
packages have been produced or published. Native external-artifact declarations,
custom dependency migration, existing-install adoption, production signing
custody and remote package acceptance remain open. CUA's producer manifest must
select the CLI explicitly: generic repository Latest may identify another CUA
product. Required bundled companions continue to obey release completeness
checks while plugin management is added.

Source authors build a native archive and prepare its exact OCI manifest locally:

```sh
aster use registry package build /path/to/plugins/plugins/aster/example \
  --output /path/to/build/example.aix --json
aster use registry package check /path/to/build/example.aix --json
aster use registry package prepare /path/to/build/example.aix \
  --repository /path/to/plugins \
  --domain development \
  --output /path/to/build/manifest.json \
  --json
```

Build reuses the native directory/archive inspector and its bounded, link-free
file inventory. It preserves Unix file modes and writes fixed ZIP timestamps,
sorted paths and a fixed compression level. A fresh native inspection must match
the source identity and expanded-file fingerprint before atomic publication.
Output must be outside the source, use the exact `.aix` suffix and not already
exist; its parent directory must exist. Native filesystem filename rules are
preserved, including explicit I/O refusal where a filename is unsupported.
Invalid source or output leaves no partial archive. This creates no
installation or trust grant and performs no signing or upload. Windows writes
regular-file mode 0644; native platform release checks remain required.

Preparation locates `plugins/<publisher>/<name>` from the validated
native package identity, checks domain/publisher admission, requires a nonempty
UTF-8 `LICENSE`, and compares the complete expanded source fingerprint with
the AIX archive. Redirected source directories refuse. It hashes archive bytes
with the existing native AIX compressed-size bound and generates the same OCI
manifest structures consumed by the downloader. The output contains exact
digest-identified bytes, without an added newline. An existing output is never
overwritten, and output inside the validated plugin source refuses. I/O failure
may leave a partial newly created output; only successful preparation provides
publishable evidence, and publication must recheck the declared digest.

The JSON result retains declared owners, native source path, package inventory,
GHCR repository and both archive/manifest SHA-256 identities. Preparation does
not install, execute, sign, upload or establish trust. The signed catalog and
real GHCR publication are separate required steps.

## Native desktop runtime plugins

Sandbox, Document and Office use the existing schema-v3 `plugin` surface,
with the single `runtime` entry pointing to `runtime.acl`. The desktop owns a
closed adapter for the three package IDs above. Its descriptor admits only a
schema version, exact component kind and native target; it does not introduce
an arbitrary executable entrypoint, download source or installation script.
The native assets retain their owning layout under `payload/`.

The Studio packaging source builds these packages with the existing native
`aster use registry package build`, `check` and `prepare` commands. It emits
AIX blobs, exact OCI manifests and publication receipts under
`target/release/runtime-components/`. Generated publication source trees live
under `repository/<target>/plugins/aster/<name>` within that build
directory, outside tracked source. Domain preparation is local evidence;
production review authority, signing and publication remain separate gates.

The client uses the same `ExtensionRegistry` lifecycle and immutable package
generations as other AIX packages, together with the official pinned Registry's
TUF/GHCR verifier and verified offline cache. The desktop build consumes an already published AIX component lock instead of
rebuilding components for each host release. The desktop pin binds version,
target, archive digest, expanded-package digest and manifest digest. A matching
published generation is reused. A full bootstrap imports its sealed bundled
bytes through `prepare_release_bundle`; a slim install downloads the native AIX.
Both paths validate the runtime descriptor and owning native membership before
publishing the generation. The plugin catalogue and runtime startup share this
installation projection. Native runtime generations and receipts stay under the
current tenant profile. Required plugins cannot be removed; Office remains optional.

Keep the authoring repository organized by domain/publisher/package. Put platform
build recipes under `recipes/<name>/<target>.acl`, outside the package source;
assemble one complete source tree for each target during the build. Platform
names and versions belong to catalog fields and artifact identities, not to the
logical package ID. Large binaries, models and images belong inside AIX/GHCR
artifacts, not in the authoring Git history.

This is a locally implemented producer/consumer path, not a publication claim.
The official repository and its initial trust root still need deployment and
fresh-install acceptance. Full bootstrap delivery remains the default until
anonymous GHCR access and the official TUF source are available without a user
having to configure a Registry manually. Slim builds require the public TUF root
metadata SHA-256 (`ASTER_PLUGIN_ROOT_SHA256`) and a released component lock
(`ASTER_RUNTIME_COMPONENTS_LOCK`); the sealed `official-plugins.acl` enables the
official source on a fresh machine without granting an unpinned fallback. Existing clients must receive the full
component-aware bootstrap before they can accept a slim update.

## Publication and trust

Publish with the native scripts in the `plugins` repository, associate the OCI package with
`https://github.com/aster-ai-lab/plugins`, and use the workflow's scoped
`GITHUB_TOKEN`. Official public packages must be readable anonymously; first
publication needs an explicit package-visibility check because GHCR defaults
new packages to private. Keep signing private keys out of Git and package
contents. The initial trust root must be established separately and pinned in
the client; later metadata updates follow the existing TUF verification,
expiry and rollback contracts.

The signed catalog now admits native `.aix` targets and the versioned GHCR
target contract. Local HTTP protocol tests cover digest-bound downloads,
resumption, redirect authorization, damaged blobs, metadata bounds, native
package preparation and cached/offline reuse. Slim release builds additionally
verify the signed catalog against the component lock and perform fresh anonymous
installation through the native SDK CLI in a temporary profile. Repository
ownership and a file extension do not replace those checks.

## References and current status

- [Krew manifests](https://krew.sigs.k8s.io/docs/developer-guide/plugin-manifest/)
  provide a useful separation between package identity, versions, platform
  selection, download verification and actual executable paths. Aster retains
  its native ACL and AIX contracts.
- [Krew custom indexes](https://krew.sigs.k8s.io/docs/user-guide/custom-indexes/)
  document explicit source identity and named-index discovery.
- [GitHub Container registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
  documents OCI support, repository association, workflow authentication,
  anonymous public access and initial private visibility.
- [ORAS artifact publication](https://oras.land/docs/1.2/how_to_guides/remote_registries/)
  demonstrates distributing file artifacts through GHCR.
- [OCI Distribution](https://github.com/opencontainers/distribution-spec/blob/main/spec.md)
  defines manifest/blob retrieval and digest addressing.
- [TUF specification](https://github.com/theupdateframework/specification/blob/master/tuf-spec.md)
  defines signed metadata updates and consistent snapshots.

The repository was initialized on 2026-11-01–02 with the public TUF root,
domain ownership, native publication scripts and daily metadata signing. Three
macOS ARM64 runtime plugins are published in GHCR with compatible 46-character
content versions and full integrity digests. The pre-release layout redesign
removes the three withdrawn 70-character experiments from the active catalog;
three available component records remain. Source and GHCR paths use the stable
`<publisher>/<name>` identity independently of domain categories.

All three GHCR packages are currently private. Fresh anonymous installation
reaches GHCR and fails with HTTP 401; the slim-release gate therefore blocks
publication until their visibility is public. Full bootstrap delivery remains
the default. The dated [verification record](evidence/desktop/2026-11-01-aix-runtime/OUTCOME.md)
contains source provenance, test results and coverage limits. The broad SDK suite
retains the same 57 fixture/content-digest failures as the committed baseline.
