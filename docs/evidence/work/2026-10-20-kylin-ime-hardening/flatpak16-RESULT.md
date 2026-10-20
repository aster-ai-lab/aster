# Actual offline Flatpak 1.6 input-method verification

The exact executables extracted from the offline package passed the GTK probe through the installed product launcher and fcitx4 module:

- Flatpak **1.6.5-0ubuntu0.5**, bubblewrap **0.4.0-1ubuntu4.1**, xdg-dbus-proxy **0.1.2-1**.
- Host X server **:120**, actual Flatpak client **:99.0**; `org.fcitx.Fcitx-99` was confirmed unowned.
- Launcher selected `GTK_IM_MODULE=fcitx` and the shipped `/app/lib/im/fcitx4/immodules.cache` through the generic fcitx alias.
- Real portal trace contains **1 CreateInputContext** and **38 ProcessKeyEvent** calls, on `/org/freedesktop/portal/inputcontext/0`.
- Probe passed Chinese `你好`, mixed English, and Escape cancellation.
- Negative control: disabling only the isolated daemon's ipcportal addon left its generic/display-specific services available, but the mapped client failed at phase 0, as expected.

`flatpak16-proof.json` binds these observations to exact package, binary, app bundle, launcher, module and probe hashes. Tested app commit: `13573ab54e364f3935a8f9686e8a0b53052c78af227104b6b58f638614c639f0`.

## Reproduce inside the owned container

The container must have the existing GNOME 50 runtime and test build/GTK/fcitx4/Xvfb dependencies. Its `/src` is Work and `/aster` is the repository.

```bash
bash /aster/target/evaluations/kylin-ime-hardening/prepare-flatpak16.sh
bash /aster/target/evaluations/kylin-ime-hardening/run-flatpak16.sh stock
bash /aster/target/evaluations/kylin-ime-hardening/run-flatpak16.sh no-seccomp
bash /aster/target/evaluations/kylin-ime-hardening/run-flatpak16.sh portal-disabled
```

On this Rosetta container, `stock` is expected to fail with `PR_SET_SECCOMP: Invalid argument`; `no-seccomp` passes; `portal-disabled` is expected to exit 1 after the bounded phase-0 timeout. The experiment corrects the container image's empty `/etc/machine-id` from its own existing D-Bus id; no id is recorded. No desktop-user state is read or copied. Profiles, bus and X server are isolated and cleaned up.

## Exact workaround boundary

`bwrap-without-seccomp.py` is an experiment-only wrapper. It expands Flatpak's argument-fd transport, removes exact `--seccomp FD` pairs, and asserts that every other semantic argument is unchanged. `bwrap-experiment.jsonl` records that assertion and the actual `/tmp/.X11-unix/X120` to `X99` bind. It adds no bus or filesystem permissions. The probe directory is an explicit test-only read-only filesystem grant. The maintained C probe is copied and only its `/probe/` fixture prefix is rewritten to that directory. The launcher and module are unmodified installed product files.

This wrapper is not shipped. These results prove Flatpak 1.6 argument construction, filtered D-Bus and GTK protocol operation under the stated workaround; they **do not prove seccomp enforcement**.

The final positive log also retained two nonfatal D-Bus credential broken-pipe warnings from short-lived control connections; the complete semantic probe still passed.

## Primary-source checks and compatibility limit

[Flatpak 1.6.5 X11 mapping and implicit portal grants](https://github.com/flatpak/flatpak/blob/1.6.5/common/flatpak-run.c): the actual client maps the X socket to X99 and grants portal method calls plus broadcasts under `/org/freedesktop/portal/*`.

[Fcitx 4.2.9.9 GTK client fallback](https://github.com/fcitx/fcitx/blob/4.2.9.9/src/lib/fcitx-gclient/fcitxclient.c): after its display-specific service is missing, it tries `org.freedesktop.portal.Fcitx` at `/org/freedesktop/portal/inputmethod`.

[Fcitx 4.2.9.7 portal paths](https://github.com/fcitx/fcitx/blob/4.2.9.7/src/frontend/ipcportal/ipcportal.h) already match this protocol. [Fcitx 4.2.9.6](https://github.com/fcitx/fcitx/blob/4.2.9.6/src/frontend/ipcportal/ipcportal.h) uses the older `/inputmethod` path. Source compatibility with 4.2.9.7 is inferred from those definitions; only daemon **4.2.9.9** was executed here. Do not claim all older daemon versions or a disabled/missing portal are supported.

## Explicitly not claimed

- Full Kylin OS, its GLib/system dependency versions, physical keyboard, full WebKit product UI or candidate-window/focus acceptance.
- Production sandbox security under the seccomp workaround.
- End-to-end execution of older fcitx daemon binaries, Sogou proprietary binaries, Wayland or ARM.
- The new frontend hardening in the already-frozen app bundle tested here. Its frontend was built before that hardening; this experiment targets the unchanged native launcher/module delivery path.
- A commit, release, update or modification to product source, staging/dist or installed product metadata by this experiment.
