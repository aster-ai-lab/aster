<a id="explicit-studio-contract-proof"></a>

# Explicit AsterDream contract proof

This independent archive declares contract version 1 and append composition for
each view. It reuses the view-commands provider assets and package identity so the
same API/native/renderer campaign proves both legacy and explicit declarations.
Each campaign installs one archive into a disposable installation. The retained
legacy archive and fixture remain unchanged.

Build with `node scripts/build-aix-proof.mjs <output.aix> view-contract`. Run the
actual archive chain with `bash scripts/test-aix-view-commands.sh <new-directory>
view-contract`. The command uses memory-backed API storage and executing documents
in JSDOM; real PostgreSQL/RustFS and platform WebView qualification are separate.
