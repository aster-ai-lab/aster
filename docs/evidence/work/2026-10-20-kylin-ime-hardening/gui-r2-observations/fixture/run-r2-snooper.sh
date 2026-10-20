#!/bin/bash
set -euo pipefail
root=/aster/target/evaluations/kylin-ime-gui
if [[ "${1:-}" != --network && "${1:-}" != --session ]]; then
  scope="$(mktemp -d "$root/run-XXXXXX")"
  printf '%s\n' "$scope" >"$root/last-run"
  echo "GUI fixture scope: $scope"
  exec unshare --net --mount -- bash "$0" --network "$scope"
fi
if [[ "$1" == --network ]]; then
  ip link set lo up
  mount --make-rprivate /
  mkdir -p "$2/flatpak-profile"
  mount --bind "$2/flatpak-profile" /root/.var/app/dev.a3s.AsterDream
  exec xvfb-run -a -n 150 -s '-screen 0 1440x1000x24 -nolisten tcp' dbus-run-session -- bash "$0" --session "$2"
fi
scope="$2"
unset WAYLAND_DISPLAY GTK_IM_MODULE GTK_IM_MODULE_FILE XMODIFIERS FCITX_DBUS_ADDRESS NO_AT_BRIDGE KYLIN_IM_BUS_OWNERS KYLIN_IM_MODULE_ROOT
unset A3S_GATEWAY_ORIGIN A3S_GATEWAY_KEY A3S_ORG_API_ORIGIN
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy NO_PROXY no_proxy
export GDK_BACKEND=x11
export XDG_CONFIG_HOME="$scope/host-config" XDG_DATA_HOME="$scope/host-data" XDG_CACHE_HOME="$scope/host-cache"
export XDG_RUNTIME_DIR="$scope/runtime"
export FLATPAK_USER_DIR=/root/.local/share/flatpak
mkdir -p "$XDG_CONFIG_HOME/fcitx" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR" "$scope/app-data" "$scope/app-config" "$scope/app-cache" "$scope/app-state"
chmod 700 "$XDG_RUNTIME_DIR"
cat >"$XDG_CONFIG_HOME/fcitx/profile" <<'PROFILE'
[Profile]
IMName=pinyin
EnabledIMList=fcitx-keyboard-us:True,pinyin:True
PreeditStringInClientWindow=True
PROFILE
instance="gui-ime-$$"
export A3S_STUDIO_INSTANCE="$instance"
python3 - "$scope" "$instance" <<'PY'
import json, os, sys
from pathlib import Path
scope=Path(sys.argv[1]); instance=sys.argv[2]
seed=scope/'flatpak-profile/data/dev.a3s.studio'/('aster-'+instance)
seed.mkdir(parents=True)
(seed/'member-session.json').write_text(json.dumps({'token':'fixture-only-kylin-ime-not-an-account','display_name':'输入法隔离测试','phone':'','avatar':'','member_id':'00000000-0000-4000-8000-000000000001'}))
(seed/'member-session.json').chmod(0o600)
(scope/'environment.json').write_text(json.dumps({key:os.environ.get(key,'') for key in ['DISPLAY','XAUTHORITY','DBUS_SESSION_BUS_ADDRESS','XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','XDG_RUNTIME_DIR','FLATPAK_USER_DIR','A3S_STUDIO_INSTANCE']}))
PY
cleanup() {
  local result=$?
  for app_instance in $(env -u LD_LIBRARY_PATH /usr/bin/flatpak ps --columns=instance 2>/dev/null); do
    [[ "$app_instance" =~ ^[0-9]+$ ]] && env -u LD_LIBRARY_PATH /usr/bin/flatpak kill --user "$app_instance" 2>/dev/null || true
  done
  kill "${app_pid:-}" "${fcitx_pid:-}" "${gateway_pid:-}" 2>/dev/null || true
  wait "${app_pid:-}" "${fcitx_pid:-}" "${gateway_pid:-}" 2>/dev/null || true
  return "$result"
}
trap cleanup EXIT
fcitx_pid=""
printf '%s\n' "${GUI_IME_DAEMON:-modern}" >"$scope/daemon-mode.txt"
case "${GUI_IME_DAEMON:-modern}" in
  modern) fcitx -v >"$scope/daemon-version.txt" 2>&1; fcitx -D -s 0 >"$scope/fcitx.log" 2>&1 & fcitx_pid=$! ;;
  old4296)
    old_prefix=/aster/target/evaluations/kylin-ime-native-review/install-4296
    env LD_LIBRARY_PATH="$old_prefix/lib" "$old_prefix/bin/fcitx" -v >"$scope/daemon-version.txt" 2>&1
    env LD_LIBRARY_PATH="$old_prefix/lib" "$old_prefix/bin/fcitx" -D -s 0 >"$scope/fcitx.log" 2>&1 & fcitx_pid=$! ;;
  none) printf 'No IM daemon control\n' >"$scope/daemon-version.txt" ;;
  *) echo 'unsupported fixture daemon' >&2; exit 1 ;;
esac
if [[ -n "$fcitx_pid" ]]; then
  display_number="${DISPLAY##*:}"
  display_number="${display_number%%.*}"
  daemon_deadline=$((SECONDS + 10))
  until gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner "org.fcitx.Fcitx-$display_number" | grep -q '(true,)'; do
    [[ "$SECONDS" -lt "$daemon_deadline" ]] || { echo 'Private fcitx daemon did not become ready' >&2; exit 1; }
    sleep 0.05
  done
fi
expected="${GUI_APP_COMMIT:?explicit final application commit required}"
[[ "$expected" =~ ^[0-9a-f]{64}$ ]] || { echo 'invalid expected commit' >&2; exit 1; }
printf '%s\n' "$expected" >"$scope/expected-commit.txt"
actual="$(flatpak info --user --show-commit dev.a3s.AsterDream)"
[[ "$actual" == "$expected" ]] || { echo 'Final application commit does not match' >&2; exit 1; }
ip -json route show >"$scope/routes.json"
export LD_LIBRARY_PATH=/aster/target/evaluations/kylin-ime-hardening/flatpak16/usr/lib/x86_64-linux-gnu
export FLATPAK_DBUSPROXY=/aster/target/evaluations/kylin-ime-hardening/flatpak16/usr/bin/xdg-dbus-proxy
export FLATPAK_BWRAP="$root/bwrap-without-seccomp.py"
extra_env=()
gateway_pid=""
if [[ "${GUI_GATEWAY_FIXTURE:-0}" == 1 ]]; then
  python3 "$root/offline-gateway.py" "$scope" >"$scope/gateway.log" 2>&1 &
  gateway_pid=$!
  gateway_deadline=$((SECONDS + 5))
  until [[ -s "$scope/gateway.json" ]]; do
    [[ "$SECONDS" -lt "$gateway_deadline" ]] || { echo 'Offline gateway failed to start' >&2; exit 1; }
    sleep 0.05
  done
  gateway_origin="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["origin"])' "$scope/gateway.json")"
  extra_env+=(--env=A3S_GATEWAY_ORIGIN="$gateway_origin" --env=A3S_GATEWAY_KEY=fixture-only-kylin-ime-not-an-account)
  python3 - "$scope" "$instance" <<'SEED'
import json, sys
from pathlib import Path
scope=Path(sys.argv[1]); instance=sys.argv[2]
gateway=json.loads((scope/'gateway.json').read_text())
seed=scope/'flatpak-profile/data/dev.a3s.studio'/('aster-'+instance)
choice={'providerId':'openai','modelId':'gpt-test'}
(seed/'providers.json').write_text(json.dumps({'providers':{'openai':{
 'enabled':True,'apiKey':'fixture-only-kylin-ime-not-an-account','baseUrl':gateway['origin']+'/v1',
 'models':[{'id':'gpt-test','name':'离线输入法测试模型','enabled':True,'chat':True}]}},
 'defaults':{'defaultModel':choice,'quickModel':choice,'translateModel':choice},'custom':[]}))
(seed/'providers.json').chmod(0o600)
project=scope/'private-project'
project.mkdir()
(project/'sentinel.txt').write_text('OWNED_OFFLINE_IME_FIXTURE\n')
preferences=scope/'flatpak-profile/data/dev.a3s.studio'/('aster-'+instance)/'preferences'
preferences.mkdir()
member='00000000-0000-4000-8000-000000000001'
(preferences/(member+'.acl')).write_text('preferences '+json.dumps(member)+' { revision = "1" preference "app.workspace.path" { value = '+json.dumps(str(project))+' } }\n')
SEED
fi
if [[ "${GUI_IME_SYNC_MODE:-0}" == 1 ]]; then extra_env+=(--env=FCITX_ENABLE_SYNC_MODE=1); fi
if [[ "${GUI_IME_SNOOPER:-0}" == 1 ]]; then extra_env+=(--env=FCITX_DISABLE_SNOOPER=0 --env=IBUS_DISABLE_SNOOPER=0); fi
if [[ "${GUI_IME_TRACE:-0}" == 1 ]]; then
  extra_env+=(--filesystem="$root/readonly-observer:ro" --env=LD_PRELOAD="$root/readonly-observer/libime-dom-observer-v2.so")
fi
/aster/target/evaluations/kylin-ime-hardening/flatpak16/usr/bin/flatpak run --user --no-documents-portal --a11y-bus --socket=x11 \
  --nofilesystem=home --filesystem="$scope" \
  "${extra_env[@]}" --env=GDK_BACKEND=x11 --env=A3S_STUDIO_INSTANCE="$instance" \
  --env=XDG_STATE_HOME="$scope/app-state" \
  --env=WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 --env=LIBGL_ALWAYS_SOFTWARE=1 \
  dev.a3s.AsterDream >"$scope/app.log" 2>&1 &
app_pid=$!
printf '%s\n' "$app_pid" >"$scope/app-launcher.pid"
echo 'Final application launched in private display/network/profile; waiting for test stop file.'
deadline=$((SECONDS + 900))
while [[ ! -e "$scope/stop" ]] && kill -0 "$app_pid" 2>/dev/null; do
  [[ "$SECONDS" -lt "$deadline" ]] || { echo 'GUI fixture deadline exceeded' >&2; exit 1; }
  sleep 0.1
done
