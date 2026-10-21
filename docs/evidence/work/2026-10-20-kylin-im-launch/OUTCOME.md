# Kylin launch selects the host input method

Result: the Kylin Flatpak command is `kylin-launch`. It chooses one input-method environment from the host session, then execs `aster-dream`. Finish metadata no longer forces `GTK_IM_MODULE=xim` on every session.

## What landed

- `apps/work/scripts/kylin-flatpak/kylin-launch.sh` reads `WAYLAND_DISPLAY` and session-bus name ownership. Wayland leaves `GTK_IM_MODULE` unset. X11 ibus sets `GTK_IM_MODULE=ibus`. X11 fcitx or fcitx5 sets `GTK_IM_MODULE=xim` and `XMODIFIERS=@im=fcitx`. fcitx5 is `org.fcitx.Fcitx5`.
- `package-bundle.sh` installs that script, sets `--command=kylin-launch`, and grants `org.fcitx.Fcitx`, `org.fcitx.Fcitx5`, and `org.freedesktop.IBus`. It does not grant `org.fcitx.Fcitx-5` and does not bake an input-method module.
- The installer completion text and `安装说明.txt` tell the desktop user to delete a stale `GTK_IM_MODULE` override. They do not tell the user to set `xim`.
- `tests/kylin_input_method.py` execs the shipped script on fixture sessions.

## Verification

- `python3 -m unittest -v tests.kylin_input_method`: 3 tests passed.
- `bash apps/work/scripts/kylin-flatpak/install-on-kylin.sh --self-test`: `install-on-kylin self-test ok`.
- `bash -n` on `kylin-launch.sh`, `install-on-kylin.sh`, and `package-bundle.sh`: passed.

## Explicitly not claimed

- No Kylin desktop, fcitx, ibus, or Flatpak was typed into. The fixture pass is not a Chinese candidate-window acceptance.
- `org.gnome.Platform//50` was not opened. ibus still depends on that runtime already containing `im-ibus`. `im-fcitx.so` was not vendored. fcitx5 on X11 with the XIM addon off still needs that addon enabled on the host.
- The `.run` installer and the Flatpak were not rebuilt. A user who already saved `GTK_IM_MODULE` in `~/.local/share/flatpak/overrides` keeps that file until they remove the keys. The new command clears those variables in its own process after Flatpak applies the override.
- ARM, Kylin Server/RPM, Desktop, the workspace browser, and the homepage composer were not changed.
