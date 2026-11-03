# Aster Box

`aster/aster-box` recipe 0.1.0 packages Box 3.2.7 from the identified Aster
workspace build. Its source identity remains `github:aster-ai-lab/Box`; the
implementation now lives in `crates/box` in the Aster workspace. The accepted
macOS ARM64 archive contains the CLI, entitled shim, matching static Linux
ARM64 guest init, native libraries and original notices. The recipe pins the
archive's exact length and SHA-256; native payloads stay outside Git.

From this repository root:

```text
python3 recipes/aster-box/darwin-arm64.py /path/to/aster-box-darwin-arm64.tar.gz --aster-use-bin /path/to/aster-use --out /path/to/build-output
```

The recipe stages `repository/plugins/operations/aster/aster-box` in the output
directory, generates `toolchain.acl`, and runs native AIX build, check and catalog
preparation. This source unit needs the qualified runtime archive; it is not a
standalone binary package. Changed runtime bytes and existing output directories
are refused before package generation. Rebuilding the runtime requires separate
qualification and updating its accepted identity.

`SOURCE.json` retains the original local candidate's source snapshot and binary
origins verbatim. Its absolute paths identify build inputs; consumers do not
use those paths. That candidate is not a clean release commit. LICENSE and the
catalog license cover the MIT Aster recipe. THIRD_PARTY_NOTICES.md and the
archive's `licenses/` retain the native components' separate terms, including
Apache-2.0, GPL-2.0-only, LGPL-2.1-only and EDK2 notices. The original native
provenance document describes the packaged Windows runtime; it does not qualify
corresponding-source redistribution of this macOS candidate.

Only macOS ARM64 package installation, version startup and lifecycle were
previously qualified. VM execution, other hosts, complete redistribution
qualification, official publication and plugin-page GUI acceptance remain open.
No upstream installer runs and no upstream release is resolved by this recipe.
