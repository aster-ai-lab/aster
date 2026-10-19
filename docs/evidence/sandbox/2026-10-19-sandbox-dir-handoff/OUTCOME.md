# Sandbox directory handoff

## Result

`discover` takes `ASTER_SANDBOX_DIR` when it is set, and otherwise the
`sandbox/` directory beside the executable. A missing companion file,
or a companion directory inside the workspace, stays a typed
`native_process_lifetime_unqualified` refusal. `mediated` still does
not consult OCI.

AsterDream and ToWork pass a qualified sibling `sandbox/` into the ACP
child only when the parent has not set `ASTER_SANDBOX_DIR`. An existing
value is inherited. A missing sibling does not invent
`~/Library/Caches/aster/sandbox`.

## Commands

From the repository root, each filter alone, `--test-threads=1`:

```
cargo test -p a3s-code-core --locked --lib companion_directory_uses_the_env_and_otherwise_the_executable_sibling -- --test-threads=1
```

1 passed, 0 failed, 0.00s. Compile 44.30s.

```
cargo test -p a3s-code-core --locked --lib each_missing_companion_file_stays_a_typed_refusal -- --test-threads=1
```

1 passed, 0 failed, 0.01s.

```
cargo test -p a3s-code-core --locked --lib companions_inside_the_workspace_are_rejected -- --test-threads=1
```

1 passed, 0 failed, 0.00s.

```
cargo test -p a3s-code-core --locked --lib validated_companion_files_keep_deny_on_oci_and_mediated_off_the_vm -- --test-threads=1
```

1 passed, 0 failed, 0.00s.

```
cargo test -p aster-dream --locked --lib sandbox_dir -- --test-threads=1
```

4 passed, 0 failed, 781 filtered, 1.64s.

```
cargo test -p a3s-studio --locked --lib sandbox_dir -- --test-threads=1
```

4 passed, 0 failed, 811 filtered, 1.52s.

`rustfmt --edition 2021` on the two core files and `rustfmt --edition 2024`
on both `acp.rs` files exited 0.

## Explicitly not claimed

No real HVF command. No OCI image was copied into either app bundle.
No AsterDream or ToWork process was restarted, so a session already
running keeps the previous refusal. Native macOS shell stays refused.
`mediated` has no mediator VM. The cache image under
`~/Library/Caches/aster/sandbox` was not used.
