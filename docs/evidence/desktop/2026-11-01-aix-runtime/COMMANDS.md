# Verification commands

Run Cargo from the repository root with pinned Rust 1.97.1 and the Studio
submodule initialized. The isolated source export used the same commands with
its root as the working directory and the existing absolute target directory.

```sh
cargo test --locked -p aster-use-extension --lib --target-dir target/studio/aster -j 2
cargo test --locked -p aster-use --lib cli::registry::aix_tests --target-dir target/studio/aster -j 2
cargo test --locked -p aster-use-core --test plugin_catalog_contracts native_aix_catalog_targets_are_admitted_without_accepting_foreign_package_formats --target-dir target/studio/aster -j 2
cargo clippy --locked -p aster-use-extension -p aster-use-core -p aster-use --all-targets --no-deps --target-dir target/studio/aster -j 2
cargo test --locked -p aster-studio --no-default-features --features tenant-aster --lib runtime_components --target-dir target/studio/aster -j 2
cargo test --locked -p aster-studio --no-default-features --features tenant-aster --lib capabilities::registry --target-dir target/studio/aster -j 2
cargo test --locked -p aster-studio --no-default-features --features tenant-aster --lib app_updater::delivery_manifest --target-dir target/studio/aster -j 2
cargo clippy --locked -p aster-studio --no-default-features --features tenant-aster --all-targets --no-deps --target-dir target/studio/aster -j 2
cargo build --locked -p aster-use --bin aster-use --target-dir target/studio/aster -j 2
CARGO_TARGET_DIR=target/studio/aster python3 apps/studio/scripts/test_runtime_components_defer.py
CARGO_TARGET_DIR=target/studio/aster python3 apps/studio/scripts/tests/studio_delivery_test.py
python3 apps/studio/scripts/tests/runtime_registry_test.py
bash -n apps/studio/scripts/package-host.sh
```

The normal `target/debug/aster-use` builder is used when `CARGO_TARGET_DIR` is
unset. Windows uses the `.exe` filename and its native archive mode semantics.
The SDK baseline comparison ran the extension suite on a separate ordinary
export of the original committed sources, then compared every failed test name.

Fresh-source installation uses the production release gate:

```sh
python3 apps/studio/scripts/verify_runtime_registry.py \
  apps/studio/resources/runtime-components.lock.json \
  e1cf8be656c072939721b765eef12a597c668fe9c33cc0de2379370d3879ae9b \
  target/studio/aster/debug/aster-use
```

Expected current result: the native client verifies the compatible catalog
record, then installation fails with `use.extension.registry_oci_invalid`,
HTTP 401, because the packages remain private. This failure is the release gate,
not a successful fresh-install qualification. The temporary profile is removed.
