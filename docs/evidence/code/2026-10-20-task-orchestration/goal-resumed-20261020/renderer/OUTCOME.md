# Frozen renderer build

Built the current renderer into the new directory:

`/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/renderer`

Exact executable command, from `apps/work`:

```sh
env -u RENDERER_VITE_PPIO_APP_SECRET -u RENDERER_VITE_AIHUBMIX_SECRET \
  DESKTOP_RENDERER_DIST=/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/renderer \
  node ./node_modules/@rsbuild/core/bin/rsbuild.js build --no-env
```

The retained Python subprocess removed those two keys from its child environment
and set only the displayed dist override; `start.json` / `result.json` retain the
actual argv and environment operations without recording secret values.

The repository's custom Rsbuild plugin/helpers contain no dotenv loading or
`process.env` write-back. The installed Rsbuild **2.2.7** CLI normally loads `.env`
before its config factory. Its existing `--no-env` flag disables that loader;
simply removing the two inherited keys would not have been sufficient. The
reviewed source hashes are retained in `environment-ingress-review.json`.
No `.env` or provider/session file was read or copied for this preparation.

Build result: **exit 0**, measured command wall time **8.829734792 s**; Rsbuild
reports **7.51 s**. `windows/main/index.html` exists. `dist-sha256.json` records
all **2,639 regular files / 76,760,505 bytes**, with **zero symlinks**; its own
SHA-256 is `6b4d6a428761166411228daaec42dc63b201cd8d702d11f05840d883027ca62e`.
The fifteen owned frontend files match their prior frozen identities and remain
unchanged before/after this build (`source-before.json`, `source-after.json`).

Four critical-dependency warnings remain in `build.log` (two direct `import.meta`
warnings and two expression-import warnings); the build succeeded. No source
repair or dependency installation was performed.

## Explicitly not claimed

No preview server, native host, GUI/CUA, Cargo command, model, VM or T01 was
started. This artifact does not establish app signing, a successful desktop
launch, native runtime identity or task performance. The root owns those gates.
