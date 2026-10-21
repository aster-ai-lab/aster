# Final frozen renderer

Built the current Work renderer into the new directory, leaving the earlier renderer intact:

`/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/renderer-final`

Exact build command, with working directory `/Users/roylin/code/aster/apps/work`:

```sh
env -u RENDERER_VITE_PPIO_APP_SECRET -u RENDERER_VITE_AIHUBMIX_SECRET \
  DESKTOP_RENDERER_DIST=/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/renderer-final \
  node ./node_modules/@rsbuild/core/bin/rsbuild.js build --no-env
```

The actual subprocess removed those two keys from its child environment and disabled Rsbuild's dotenv loader before the config factory. Current custom plugin/helper sources still do not reload dotenv or write those environment keys. No secret values, `.env`, provider configuration, or user session files were inspected or copied. See `environment-ingress-review.json` and `start.json`.

Result: **exit 0**, command wall time **11.630260167 s**, Rsbuild **9.91 s**. Four existing critical-dependency warnings plus config/route warnings remain in `build.log`.

## Frozen identities

- `inputs-before.json` / `inputs-after.json`: **8,265 scoped input files**, byte-identical before/after. The manifests include Work sources, local package sources, scripts/config/lockfiles, migrated Flow UI sources, Office/form inputs, and the explicitly listed installed build/copy inputs. Both manifest SHA-256 values are `8ab22ef20756bef13df24da490890be1b85dac0aaa0dbaf26040250c4b3324cd`.
- `input-scope.json` states exact roots and exclusions. This is not a whole-`node_modules` content freeze.
- `dist-sha256.json`: all **2,639 regular files / 76,760,505 bytes**, **0 symlinks**, with `windows/main/index.html` present. Manifest SHA-256: `aa9ca9f6022e235d3fe985e9338a731b13d643a73aa17f721b7407f92c89cd5a`.
- `previous-renderer-before.json` / `previous-renderer-after.json`: the earlier `renderer` directory remains unchanged. `comparison.json` additionally confirms every relative file and byte digest in the new build equals the earlier renderer; no path content differs.

## Flow migration check

The current Rsbuild alias helper and `tsconfig.web.json` point to `crates/flow/packages/ui/src`. All five Flow alias targets exist; their exact resolved paths are in `flow-alias-targets.json`. Current Work value imports from `harnessPlaygroundGraph.ts` resolve via `@a3s-lab/flow-ui` to the migrated `index.ts`; the actual production build completed without module resolution errors.

The alias table also retains four absent legacy `@a3s-lab/ce` distribution targets. They were not required by this current production build graph. This result does not claim a full Flow React designer build or runtime exercise. No source was changed to repair or remove those aliases.

## Preview handoff — not executed

Working directory: `/Users/roylin/code/aster/apps/work`.

```sh
env -u RENDERER_VITE_PPIO_APP_SECRET -u RENDERER_VITE_AIHUBMIX_SECRET \
  DESKTOP_RENDERER_DIST=/Users/roylin/code/aster/target/development-stage/task-optimization-r2-20261020/renderer-final \
  node ./node_modules/@rsbuild/core/bin/rsbuild.js preview --no-env --host 127.0.0.1 --port 4318 --strict-port
```

Serve root: the `renderer-final` directory above. Entry URL: `http://127.0.0.1:4318/windows/main/index.html`; entry file: `renderer-final/windows/main/index.html`. The current server config has `open: false`; `--strict-port` refuses an occupied port. The root must arrange the matching external-renderer port for its staged native host. `preview-plan.json` records the same parameters.

No preview server, native host, GUI, Cargo command, model, VM, dependency installation, source repair, or T01 task was started. This evidence establishes a renderer build and file identities, not a signed app launch or task performance.
