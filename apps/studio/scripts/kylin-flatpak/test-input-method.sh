#!/usr/bin/env bash
# Native protocol test in the Linux packaging container. No desktop user state is read.
set -euo pipefail
app_files="$(realpath "${1:?usage: test-input-method.sh APP_FILES RUNTIME_FILES}")"
runtime_files="$(realpath "${2:?runtime files are required}")"
script_dir="$(cd "$(dirname "$0")" && pwd)"
scope="$(mktemp -d)"
trap 'rm -rf "$scope"' EXIT
mkdir -p "$scope/probe"
cc -O2 -Wall -Wextra -Werror -Wno-deprecated-declarations "$script_dir/gtk_input_probe.c" \
  -o "$scope/probe/gtk-ime-probe" $(pkg-config --cflags --libs gtk+-3.0)
cc -O2 -Wall -Wextra -Werror "$script_dir/fcitx_client_protocol_test.c" \
  -o "$scope/probe/fcitx-client-protocol-test" $(pkg-config --cflags --libs gio-2.0 gmodule-2.0)
timeout 30 "$scope/probe/fcitx-client-protocol-test" "$app_files/lib/im/fcitx4/libfcitx-gclient.so.1"
cp /usr/bin/fcitx-remote "$scope/probe/fcitx-remote"
mkdir -p "$scope/probe/remote-libs"
cp /usr/lib/x86_64-linux-gnu/libfcitx-config.so.4 \
  /usr/lib/x86_64-linux-gnu/libfcitx-utils.so.0 "$scope/probe/remote-libs/"
python3 - "${app_files}/../metadata" >"$scope/talk-names" <<'PY'
import configparser, sys
metadata = configparser.ConfigParser(interpolation=None)
metadata.optionxform = str
metadata.read(sys.argv[1])
for name, policy in metadata['Session Bus Policy'].items():
    if policy != 'talk':
        raise SystemExit('unsupported test bus policy: ' + policy)
    print(name)
PY
xvfb-run -a -n 100 dbus-run-session -- bash -euo pipefail -s -- "$scope" "$app_files" "$runtime_files" <<'RUN'
scope="$1"
app_files="$2"
runtime_files="$3"
unset WAYLAND_DISPLAY KYLIN_IM_BUS_OWNERS KYLIN_IM_MODULE_ROOT FCITX_DBUS_ADDRESS
unset IBUS_ENABLE_SYNC_MODE FCITX_ENABLE_SYNC_MODE IBUS_DISABLE_SNOOPER FCITX_DISABLE_SNOOPER
unset IBUS_NO_SNOOPER_APPS FCITX_NO_SNOOPER_APPS
export GDK_BACKEND=x11
export XDG_CONFIG_HOME="$scope/host-config"
export XDG_DATA_HOME="$scope/host-data"
export XDG_CACHE_HOME="$scope/host-cache"
export XDG_RUNTIME_DIR="$scope/run"
mkdir -p "$XDG_CONFIG_HOME/fcitx" "$XDG_RUNTIME_DIR" "$scope/client-config"
chmod 700 "$XDG_RUNTIME_DIR"
dbus-uuidgen --get >"$scope/machine-id"
cat >"$scope/fonts.conf" <<'FONTS'
<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig><dir>/app/share/fonts</dir><dir>/usr/share/fonts</dir><cachedir prefix="xdg">fontconfig</cachedir></fontconfig>
FONTS
cat >"$XDG_CONFIG_HOME/fcitx/profile" <<'PROFILE'
[Profile]
IMName=pinyin
EnabledIMList=fcitx-keyboard-us:True,pinyin:True
PreeditStringInClientWindow=True
PROFILE
cleanup() {
  local status=$?
  kill "${unrelated_pid:-}" "${proxy_pid:-}" "${fcitx_pid:-}" 2>/dev/null || true
  wait "${unrelated_pid:-}" "${proxy_pid:-}" "${fcitx_pid:-}" 2>/dev/null || true
  if [[ "$status" != 0 ]]; then tail -n 30 "$scope/fcitx.log" "$scope/proxy.log"; fi
  return "$status"
}
trap cleanup EXIT
# The product must ignore an unrelated live address, not only a dead socket.
dbus-daemon --session --nofork --nopidfile --address="unix:path=$scope/unrelated-bus" >"$scope/unrelated-bus.log" 2>&1 &
unrelated_pid=$!
fcitx -D -s 0 >"$scope/fcitx.log" 2>&1 &
fcitx_pid=$!
# Flatpak also grants portal calls and broadcasts independently of metadata.
talk_args=(--filter '--call=org.freedesktop.portal.*=*' '--broadcast=org.freedesktop.portal.*=@/org/freedesktop/portal/*')
while IFS= read -r name; do talk_args+=("--talk=$name"); done <"$scope/talk-names"
xdg-dbus-proxy "$DBUS_SESSION_BUS_ADDRESS" "$scope/proxy-bus" "${talk_args[@]}" >"$scope/proxy.log" 2>&1 &
proxy_pid=$!
display_number="${DISPLAY##*:}"
display_number="${display_number%%.*}"
deadline=$((SECONDS + 10))
until [[ -S "$scope/proxy-bus" && -S "$scope/unrelated-bus" ]] && dbus-send --session --reply-timeout=1000 \
  --dest=org.freedesktop.DBus --print-reply /org/freedesktop/DBus \
  org.freedesktop.DBus.NameHasOwner "string:org.fcitx.Fcitx-$display_number" | grep -q 'boolean true'; do
  [[ "$SECONDS" -lt "$deadline" ]] || { echo 'IME daemon/proxy did not become ready' >&2; exit 1; }
  sleep 0.05
done
for mode in host-display flatpak-display; do
  display_args=()
  if [[ "$mode" == flatpak-display ]]; then
    cookie="$(xauth list "$DISPLAY" | awk 'NR == 1 {print $3}')"
    [[ "$cookie" =~ ^[0-9a-fA-F]{32}$ ]] || { echo 'missing Xvfb authentication cookie' >&2; exit 1; }
    touch "$scope/mapped-xauthority"
    xauth -f "$scope/mapped-xauthority" add :99 MIT-MAGIC-COOKIE-1 "$cookie"
    display_args=(--tmpfs /tmp/.X11-unix
      --ro-bind "/tmp/.X11-unix/X$display_number" /tmp/.X11-unix/X99
      --setenv DISPLAY :99.0 --setenv XAUTHORITY "$scope/mapped-xauthority")
  fi
  echo "kylin-ime: $mode (host DISPLAY=$DISPLAY)"
  timeout 25 bwrap --die-with-parent --dev-bind /dev /dev --proc /proc \
    --tmpfs /run --tmpfs /var --bind /tmp /tmp \
    "${display_args[@]}" \
    --ro-bind "$runtime_files" /usr --ro-bind "$runtime_files/etc" /etc \
    --ro-bind "$scope/machine-id" /etc/machine-id \
    --symlink usr/lib /lib --symlink usr/lib64 /lib64 --symlink usr/bin /bin --symlink usr/sbin /sbin \
    --ro-bind "$app_files" /app --ro-bind "$scope/probe" /probe --chdir /app \
    --setenv XDG_CONFIG_HOME "$scope/client-config" \
    --setenv DBUS_SESSION_BUS_ADDRESS "unix:path=$scope/proxy-bus" \
    --setenv FCITX_DBUS_ADDRESS "unix:path=$scope/unrelated-bus" \
    --unsetenv LD_LIBRARY_PATH \
    --setenv KYLIN_IME_HOST_DISPLAY "$DISPLAY" \
    --setenv KYLIN_IME_CONTROL_BUS_ADDRESS "$DBUS_SESSION_BUS_ADDRESS" \
    --setenv FONTCONFIG_FILE "$scope/fonts.conf" --setenv NO_AT_BRIDGE 1 \
    --setenv KYLIN_LAUNCH_TARGET /probe/gtk-ime-probe \
    -- /app/bin/kylin-launch
done
RUN
