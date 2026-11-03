# Repository layout and maintenance

The stable package identity is `<publisher>/<name>`. Source is organized as
`plugins/<domain>/<publisher>/<name>/`. A domain describes what a plugin does
and who reviews its publication. Moving a plugin to another admitted domain
moves its source directory; its package ID, catalog identity and GHCR repository
stay stable. The native SDK receives the domain explicitly and checks
`domains.acl`; it never guesses a category from a name or directory.

The domain hierarchy is Aster's authoring policy. Artifact references retain
the digest and size contract from [OCI content descriptors](https://github.com/opencontainers/image-spec/blob/main/descriptor.md).
Signed role files retain the separate layout defined by [TUF](https://theupdateframework.github.io/specification/latest/#repository-layout).
Review ownership follows [GitHub CODEOWNERS](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners),
with real repository permissions; moving source does not rotate keys or rewrite
published content identities.

| Path | Maintained by | Contract |
| --- | --- | --- |
| `domains.acl` | Domain maintainers | Explicit owner/publisher admission, parsed by native `aster-acl` |
| `plugins/<domain>/<publisher>/<name>/` | Plugin maintainers | One source unit per package in its admitted primary domain; README and licenses stay with it |
| `recipes/runtime/<target>.py` | Runtime maintainers | Pinned inputs and an executable build recipe for a qualified target |
| `recipes/<name>/<target>.py` | Acquisition maintainers | Stages qualified embedded runtime inputs outside Git and runs native package checks |
| `recipes/<name>/<target>.json` | Acquisition maintainers | Native catalog input for the explicitly supported source target |
| `registry/catalog/<publisher>/<name>/<version>/<channel>/<target>.json` | Native publisher | One generated release record for the complete selection identity |
| `registry/locks/runtime/<target>.json` | Runtime publisher | The currently selected component set for desktop packaging |
| `registry/metadata/` | TUF signer | Generated signed metadata consumed by clients |
| `registry/targets/` | Native publisher | Exact hash-prefixed MCP planning bytes, separately bound by TUF |
| `registry/root.sha256` | Root initialization/rotation | Public trust anchor; never an online refresh output |
| `scripts/` | Registry maintainers | Small operational entry points and their shared catalog/signing code |
| `skills/<skill-name>/` | AI integration maintainers | Consumer instructions and bounded evaluation prompts; no native payloads |
| `tests/` | Registry maintainers | Storage, signing-input and selected-lock contract checks |
| `.github/workflows/` | Repository maintainers | Read-only validation and a separate secret-bearing refresh job |

`.github/CODEOWNERS` names an actual repository maintainer for code review;
domain owner identifiers do not grant GitHub repository or package permissions.
`.gitattributes` keeps source text LF and preserves signed metadata as exact bytes.

## Plugin source and build ownership

Keep each plugin's manifest, source, assets and tests together when it has
handwritten extension sources. Create optional subdirectories only when used.
Do not duplicate one package across domains, add per-version source copies,
or commit native payloads alongside the source. To move a package, admit its
publisher in the destination domain, move the complete directory and prepare
with that explicit domain. Preparation checks that exact directory and archive.

The three runtime plugins currently contain documentation and licenses here.
Their generated `aster-use-extension.acl`, `runtime.acl` and payload membership
are owned by Aster's `apps/studio/scripts/package_runtime_components.py`, using
the owning native crates and `studio-runtime-files.txt`. The shared target
recipe invokes that producer, which stages a complete source tree outside Git
and verifies it against the archive through the native SDK. These directories
are not presented as independently buildable handwritten implementations.
The producer lives in the private Studio submodule; building needs an authorized
Aster checkout, while consuming published packages does not.

`aster/aster-ocr` owns a native manifest template here. Its public producer is
`crates/ocr/scripts/package_plugin.py` in Aster. The canonical OCR Skill stays
beside its Rust implementation at `crates/ocr/skills/aster-use-ocr`; the producer
copies it into the AIX. Do not maintain a second OCR Skill or a separate model
installer in this repository. OCR content versions include the manifest template,
Skill, runtime descriptor, executable, libraries, models, and notices.

External tool sources keep one `aster-use-extension.acl` and one `toolchain.acl`
per package. Schema-v2 declares each supported native platform in a labelled
`platform` block. The installer validates every archive declaration and selects
the exact host; there is no per-platform source-directory copy. One AIX may have
multiple target-specific catalog records, each prepared by the native publisher.
Upstream notices stay byte-identical. A notice that differs for a target lives
under `upstream-licenses/<target>/`; `.gitattributes` disables text conversion for
these files. Source target declarations are distinct from native qualification.

`aster/babeldoc-stream` keeps its manifest, launcher, exact dependency lock,
provenance and licenses in its source unit. Its target recipe owns the generated
`toolchain.acl` and embedded runtime payload. The recipe admits only the fixed
qualified archive, stages the complete source under the explicit output
repository, and invokes native build/check/prepare there. Do not hand-edit the
generated declaration or commit the runtime archive. Rebuilding the runtime
itself is a separate qualification; the source recipe does not silently resolve
new wheels or replace its accepted input.

`aster/aster-box` follows the same embedded-runtime contract in the explicitly
admitted `operations` domain. Its target recipe generates `toolchain.acl` from
the fixed accepted Box archive; the source unit retains the original local
build provenance and native notices. The archive includes the CLI, entitled
shim, static Linux guest init and matching native libraries. Package checks and
version startup do not qualify VM execution or corresponding-source publication.

`recipes/runtime/darwin-arm64.py` describes the current approved bootstrap
input: a digest-pinned desktop archive. It qualifies only macOS ARM64. Add a
new target recipe after its native libraries, ABI, files and runtime behavior
have been checked. Do not infer Linux, Kylin, Windows or other CPU support
from a similar directory name. Do not duplicate the producer's component
membership in a second recipe format or parser.

## Catalog and lock lifecycle

The catalog includes the publisher namespace to prevent equal plugin names
from colliding. Version, channel and target are separate path components;
business domains stay in the record's categories. A channel may select
multiple immutable versions, so it is not a single mutable release file.
The publisher and signer share one path function. The signer recursively
loads records and refuses misplaced records, duplicate TUF targets and an
empty catalog. CI compares these inputs with the current signed target set.

After release, retain catalog versions needed by supported clients and mark
withdrawn releases explicitly. Never replace a released archive under the
same version. Unreleased bootstrap experiments need not remain in the active
catalog; their changes remain in Git history. The old document bootstrap that
embedded OCR models and ONNX Runtime has been retired from the active catalog.
The selected document pack contains only PDF/font resources; OCR is its own package.

A runtime lock is a generated selection for one target, not another package
catalog or a build recipe. The desktop release copies and seals its selected
lock. The Aster parent pins this repository's exact Git revision; `main` is
not an immutable release identity. Update catalog records, locks and their
signed metadata in the same publication commit. Historical release inputs
remain identifiable by commit and by their archive/package/manifest digests.

## Signed metadata and binary storage

`registry/metadata/` follows the client's TUF repository contract. It contains
the current role aliases and versioned roots, targets and snapshots. Do not
nest role files by publisher, hand-edit signed JSON, remove generations as
cosmetic cleanup, or reset versions. Metadata paths and consistent snapshots
are protocol decisions: see the [TUF specification](https://theupdateframework.github.io/specification/latest/).

The logical package target is:

```text
extensions/<publisher>/<name>/<version>/<channel>/<target>/oci-ghcr-v1/<manifest-sha256>/<name>.aix
```

Its OCI repository is `ghcr.io/aster-ai-lab/plugins/<publisher>/<name>`.
Installation uses the signed immutable manifest and AIX digests. Tags aid
discovery; neither a business category nor a mutable tag defines artifact
identity. Associate packages with this GitHub repository and qualify anonymous
pull before claiming public first-run installation.

An executable MCP catalog record also names a native `planning-v1.json` target.
The signer verifies its exact length and digest and signs it without catalog
custom fields. With consistent snapshots, its hosted path prefixes the hash to
the complete logical target name: `registry/targets/<sha256>.extensions/...`.
Native SDK preparation binds its executable, arguments, activation, permission
digest, and archive/package/manifest identities. Never hand-author this bundle.

Native archives, models, extraction trees, credentials and private keys stay
out of Git. Build candidates use an explicit output directory, normally under
the parent checkout's `target/`. Root keys remain offline/in Keychain; online
keys are CI secrets. No `keys/`, binary cache, release-copy tree or unused
future target folders are added here. If catalog growth reaches the native
metadata byte/work limits, introduce measured TUF delegation and hosting
changes together; a deeper source directory does not shard signed metadata.
