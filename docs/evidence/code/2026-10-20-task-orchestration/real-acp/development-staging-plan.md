# Development staging plan — read-only preparation

Status: plan frozen; no app, binary, signature, asset, user configuration or process was changed by this preparation. No app was launched. Execute only after the paired build and frozen matrix release their timing/build windows.

Subsequent execution has prepared only a separate app's public metadata/icon
and a credential-free, mode-0600 capability ACL. The app contains no executable,
has not been signed or launched, and its hardware/adaptive selection remains
provisional until qualification. Exact paths and state are in
`development-stage-preparation.json`. The fresh twelve-file final CUA workspace
is recorded in `t027-input-preparation.json`; no model task has started there.

## Existing identities and the stale companion path

The current development process was PID 83498 at `/Users/roylin/Library/Application Support/dev.a3s.desktop/aster/AsterDream.app/Contents/MacOS/AsterDream`. The parent CUA observation reports `cgWindowNotFound` despite that PID being alive. A process existing is not a visible-window qualification.

| Current development file | SHA-256 |
| --- | --- |
| `Contents/MacOS/AsterDream` | `5ef74076bfee2c6a762dbfb910f5e08f2dae62739f63155a0b4a060f30472161` |
| `Contents/MacOS/a3s-code-acp` | `2771e8c39a09dccae15c294eead79539a30cca77a825add1007f1a7689377ac6` |
| `Contents/MacOS/a3s-office` | `35e9dbc647238444c51b42d3fb0ed544725ac25daf9cfe0d4465e7d93bf8db33` |

This development bundle is 0.1.0, identifier `dev.a3s.desktop.aster`. It is ad hoc signed, but strict deep verification currently fails with `invalid destination for symbolic link in bundle`. Its `Contents/MacOS/sandbox` is a link to `/Users/roylin/Library/Caches/aster/sandbox/dev-20261018-core-bash`.

`/Applications/AsterDream.app` is a separate Developer-ID-signed 0.1.0 release (`dev.a3s.desktop`), and its strict verification passed. The existing `target/release/bundle/macos/AsterDream.app` is another 0.1.1 build with older companion identities. Neither is the final paired development build; neither should supply the new development stage.

There is a concrete stale-companion trap. `macos_app.rs:177–229` refreshes only the main executable, Info.plist and icon; it leaves existing ACP/Office files beside it. `agent_plane/launcher.rs:65–75` prefers a sibling ACP, and `office_tool.rs:241–263` prefers a sibling Office binary. Therefore refreshing only the main binary through `just work` does not qualify the new three-piece execution path.

## Minimal chosen route

Keep the complete selected sandbox **external and unchanged**. Create a new independent app containing only fresh paired Work/ACP/Office binaries and public bundle metadata/icon. Set the explicit selected sandbox path through this new bundle's `LSEnvironment` dictionary. Keep the original full app at its original path for rollback. Do not replace `/Applications`, the canonical development app, the original cache or either frozen T013 asset tree.

Apple documents `LSEnvironment` as a dictionary of string keys and string values that defines the app's launch environment. The archived reference explicitly limits this mechanism to launches through Launch Services; direct command-line execution does not apply those values. This is per-app bundle metadata, not a global launchctl change. [Current Apple LSEnvironment reference](https://developer.apple.com/documentation/bundleresources/information-property-list/lsenvironment), [Apple Launch Services keys reference](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/LaunchServicesKeys.html).

The exposed CUA entry point documents `cua.getApp(path)` but does not document an environment argument. It has not been proven here that its actual launch goes through Launch Services. That is an explicit final observation gate, not an assumption: CUA must observe the exact staged process, and its first ACP/runtime receipt must prove the selected configuration and asset source before accepting a task result. If the dictionary is not effective, stop the staged process and resolve the launch mechanism; do not silently accept fallback assets or alter global environment.

A complete copy of the Linux guest tree inside the app was considered and rejected for this stage. The frozen hardware tree has 353 symlinks, including guest-absolute `/bin/busybox` links. Copying or dereferencing those to make a macOS resource seal pass could change guest meaning or frozen identity. No such transformation or trial staging is authorized by this plan. A link from inside the app to an external tree also recreates the current strict-signature failure.

## Ordered execution after matrix release

The completed build owner subsequently reported final paired ACP `fa611e2fe6404208d509fe1885e7f26cf346e169bffb88722b39bddc414fae08` and Work host `224dafa1a87ab921ae8bc4ee8f07b090538189ac554d39e52f3809353ffef29a`, with 44 frozen source paths unchanged. Exact Office/Aster identities and build commands are in `../final-build/builds.json`. These later build identities are not relabelled as the earlier T022-tested binary.

1. Freeze the **finished paired build**: record hashes/size/architecture of `target/debug/aster-dream`, `target/debug/a3s-code-acp` and `target/debug/a3s-office`, the root source manifest and selected sandbox manifest. Use the completed build owner's identities, not an earlier T022 hash if a later paired build changed it. The T022 five-case ACP was `8e4d312fb2dc6b4e634e472ea87f48e8a8f749050d5a34f774ef8ee36f1ffdaf`; its scope is recorded separately.
2. Create a new path such as `/Users/roylin/code/aster/target/development-stage/task-orchestration-20261020/AsterDream-TaskOrchestration-Dev.app`. Copy the three freshly frozen executables to `Contents/MacOS/AsterDream`, `Contents/MacOS/a3s-code-acp` and `Contents/MacOS/a3s-office`. Create fresh Info.plist/icon resources, with `CFBundleExecutable=AsterDream`, version 0.1.1 and a distinct staging identifier such as `dev.a3s.desktop.aster.task-orchestration-dev`. No sandbox symlink is added. Do not copy the tenant support directory, providers, database, session state or credentials. The original app itself remains in place, so no credential-containing support backup is necessary.
3. Add `LSEnvironment` with one required string: `ASTER_SANDBOX_DIR=<absolute selected, frozen complete asset directory>`. If the matrix qualifies the hardware tree, its current path is `/Users/roylin/code/aster/target/t013-assets-hardware`, with signed host `90e6a3e1c365948a659de6703c47af7f8c0ca413987ed8200aabcd72687e7779` and shim `11d1e608df61b08433b7bb0834953b2cfd517e5a0d0a9c1e93f977301bc7ea0a`; verify against `t013-assets-hardware.json` rather than choosing on microbenchmark speed alone. Do not re-sign those frozen assets or change the shim's HVF entitlements. No `A3S_CONFIG` override is needed for an inherited/default profile. If the final selected deployment requires a different profile or retrieval setting, use only the existing trusted ACL mechanism: a separate, credential-free host ACL with the already supported `tool_presentation`/`workspace_retrieval` blocks and a per-app `A3S_CONFIG` entry. Verify its projection into the actual launch ACL; do not copy a user's full provider ACL or introduce another config format.
4. Verify signatures on the copied companions. Ad hoc sign the new app with its distinct staging identifier, then run `codesign --verify --deep --strict <new.app>` and record final post-sign byte identities. Signing can change main executable bytes; compare the actual launched image to the **post-sign stage hash**, while retaining the original paired-build hash as provenance. Do not recursively re-sign external OCI assets. Validate the external host/shim independently with their existing signatures and manifest hashes.
5. Prepare the renderer outside the timing window. A debug Work binary uses loopback `4318` by default (`lib.rs:660–682`), while `scripts/desktop-shell.sh` normally selects Work port 4320. Read-only observation found listeners on both ports whose cwd is this checkout's `apps/work`; that proves ownership, not current frontend-byte freshness. Reuse only an owner-verified fresh Work renderer. If choosing 4320, put `A3S_DESKTOP_RENDERER_PORT=4320` in this new app's per-app dictionary before signing. The existing Rsbuild build/preview graph and `desktop-shell.sh` describe preparation, but the latter also builds/launches and must not be run during matrix timing. Never open a different product's port to make the window appear.
6. Recheck the old development PID's executable and settle only that exact primary instance normally, as authorized by the parent. The distinct staging bundle identifier avoids CUA matching the old bundle by id; the fixed tenant storage slug is still `aster`, so a live old primary can still hold the product's single-instance lock. Then call CUA `getApp(new absolute .app path)` in the background, with no activation, AppleScript/UI injection or global launchctl/environment edit. Observe an actual window, exact new mapped executable path and post-sign SHA; confirm actual ACP maps the staged sibling and Office resolves the staged sibling. Verify generated trusted ACL/profile and a bounded actual runtime receipt showing the selected external asset path. A live PID or a version label alone does not pass this gate.

## Why the new bundle does not reinstall itself

`run()` calls `macos_app::relaunch_into_tenant_bundle`, but `macos_app.rs:44–45` immediately returns for an app executable. `is_app_executable` at lines 366–378 only requires the path structure `*.app/Contents/MacOS/<executable>`. It does not require the canonical support path or a particular bundle identifier. Therefore a real Mach-O copied into the independent staged app starts in place and bypasses `spawn_bundle`/`install_bundle`; it will not copy itself back over the original canonical app. This is a source-verified conclusion; the final CUA mapped-path check remains required.

The new Info.plist identifier does not change the data-root rule: `tenant.rs` fixes the primary storage slug to `aster`, and `data_root.rs` keeps its existing per-tenant support location. No existing user data is migrated or copied by staging. The same primary lock is why old-process settlement is explicit.

## Existing commands and rollback

The root `just build-work` correctly builds ACP and Office before Work; the paired Aster/ACP/Office build remains the build owner's responsibility. `apps/work/src-tauri/build.rs` also publishes companions into `resources/agent`, but this stage copies the exact finished target binaries directly. `apps/work/scripts/package-host.sh` is the existing full renderer/release/bundle/Developer-ID path; it is not required to replace the development app or manufacture a release for this CUA test.

Rollback is limited to process selection: settle the exact staged process, then reopen the untouched original canonical app. There is no app-directory replacement to undo, no copied provider/session state, and no shared environment to restore. The original development app's existing strict-signature failure is recorded rather than falsely declaring rollback signature-qualified. The signed `/Applications` release stays independent. Keep the new stage/evidence directory for review; do not delete original binaries or frozen assets.
