# AIX conditional contribution qualification package

Build with `node scripts/build-aix-proof.mjs /tmp/context-visibility.aix context-visibility`
from `apps/desktop`. The package reuses the executable tree and mini-app fixtures.
It declares dark ActivityBar resources, a locale-specific tree, a light panel
and mini app, and an unconditional editor. Conditions observe host context;
they never authorize native resource access.

The API test publishes and downloads these exact bytes through member pins.
The native test installs the archive, projects conditions, verifies scripts
and revokes asset tickets on uninstall. The renderer consumes that native
catalogue without rewriting conditions, executes the mini app and verifies
conditional withdrawal while keeping its editor sibling mounted.

Run `bash scripts/test-aix-context-visibility.sh <new-evidence-directory>`.
This local chain uses the memory API adapter and JSDOM document scripts;
real PG/RustFS and supported platform WebViews need separate acceptance.
