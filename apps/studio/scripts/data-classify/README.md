# Data schema and migration generators

This tool owns the source definitions for Preference and BootConfig schemas and the supported v1-to-v2 migration mappings. It uses Node.js built-ins and needs no separate dependency install.

## Sources

- `data/classification.json`: legacy keys classified by target data system, including simple Preference and BootConfig definitions.
- `data/target-key-definitions.json`: complex mappings and new product Preference keys without a v1 source.
- `scripts/generate-boot-config.js`: additional BootConfig definitions in `MANUAL_BOOT_CONFIG_ITEMS`.

## Generate

From the repository root:

```sh
pnpm data:generate
```

To run an individual generator, use `npm run generate:preferences` or `generate:boot-config` in this directory.

The pipeline writes these files; edit the sources above rather than these outputs:

- `src/shared/data/preference/preferenceSchemas.ts`
- `src/shared/data/bootConfig/bootConfigSchemas.ts`
- `src-tauri/src/preference_keys.generated.rs` (ordinary Preference keys admitted by the native store)

Check the native key projection without rewriting schemas:

```sh
node scripts/data-classify/scripts/generate-preferences.js --check-native
```

After generation, inspect the diff. A relocation must preserve keys, types, defaults, and migration mappings. Run `pnpm lint` and the tests covering any intended schema or migration behavior change.

The retired inventory extraction and validation scripts are no longer part of this tool. Separating permanent product definitions from migration inputs remains tracked in [#19192](https://github.com/CherryHQ/cherry-desktop/issues/19192).
