# Kylin launch loads the fcitx5 GTK3 module

Result: X11 sessions that own `org.fcitx.Fcitx5` now point GTK at the bundled fcitx5 GTK3 module. The launcher no longer exports an X input-method module. The v0.1.1 offline installer was built with that module. No Kylin desktop was typed into.

## What landed

- `kylin-launch.sh` clears `GTK_IM_MODULE`, `GTK_IM_MODULE_FILE`, `XMODIFIERS`, and `QT_IM_MODULE`, then selects one row: Wayland leaves the module unset; X11 fcitx5 with `module-id` and `immodules.cache` sets both GTK variables; X11 ibus-only sets `GTK_IM_MODULE=ibus`; fcitx4-only or a missing fcitx5 cache sets neither variable and writes `kylin-im:` to stderr.
- `build-immodule.sh` builds fcitx5-gtk 5.1.7 (`92d7a5e3ed9eac8c8173983723013816f4706dddc3f019b4bb5c76c48cf4dd92`) in `aster-kylin-build:24.04` with GTK3 only, GIR off, and the key snooper off. Output is `apps/work/dist/kylin/immodules/fcitx5/`.
- `package-bundle.sh` installs that module at `/app/lib/im/fcitx5/`, rewrites the cache path, and refuses to finish when `ldd` cannot resolve `libgtk-3.so.0` or the runtime cache has no `ibus` entry. On this Mac, `flatpak build` cannot install a seccomp filter (`prctl(PR_SET_SECCOMP)` returns `EINVAL`). The same installed `org.gnome.Platform//50` tree is checked through `bwrap` without that filter. The shipped finish args do not disable the sandbox.
- `immodule_cache.py` rewrites the query path. README, `安装说明.txt`, and `install-on-kylin.sh` describe the fcitx5 module and the fcitx4 gap.

## Verification

- `python3 -m unittest -v tests.kylin_input_method`: 6 tests passed.
- `bash apps/work/scripts/kylin-flatpak/install-on-kylin.sh --self-test`: `install-on-kylin self-test ok`.
- `bash -n` on `kylin-launch.sh`, `build-immodule.sh`, and `package-bundle.sh`: passed.
- `bash apps/work/scripts/kylin-flatpak/build-immodule.sh`: exit 0. `module-id` is `fcitx`. The cache also lists `fcitx5`. Rewriting the cache produces `/app/lib/im/fcitx5/im-fcitx5.so`.
- `ldd` inside `aster-kylin-build:24.04` resolved `libgtk-3.so.0` and reported no `not found` lines.
- Package log `target/packages/aster-0.1.1-kylin-v10-x86_64-20261020-0627/package-bundle.log` records `kylin-package: fcitx5 module links libgtk-3.so.0 and the runtime cache has ibus`. `install-on-kylin.sh --self-test` printed `install-on-kylin self-test ok`. `pack-onefile.sh` printed `onefile-ok`.
- Artifact: `target/packages/aster-0.1.1-kylin-v10-x86_64-20261020-0627/aster_0.1.1_kylin-v10_x86_64.run`, 570,209,638 bytes, SHA-256 `e4add14e1136b1fa5dd0c5a0dd5054abc7b453d72175f105219d15b821f580eb`. App commit `6deb8b8e59abddc24ac1a237936f61bd34a254369a364cc2581d5ea5d02c9322`. Metadata command is `kylin-launch`. No `GTK_IM_MODULE`, `QT_IM_MODULE`, or `XMODIFIERS` in finish metadata. Talk names include `org.fcitx.Fcitx5` and `org.freedesktop.IBus`. Runtime and GL commits match the 0.1.0 bundles. Working tree at `18144aab129d` plus local changes. `main` was not pushed.

## Explicitly not claimed

- No Kylin desktop, fcitx5 daemon, or homepage field was typed into. A built `.run` is not Chinese candidate-window acceptance.
- fcitx4 and Sogou sessions are unsupported. The GPL fcitx4 GTK frontend was not vendored.
- Wayland, ARM, Kylin Server, RPM, and the Locale extension were not changed. The homepage composer was not changed.
- `main` was not pushed.
