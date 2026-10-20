#!/usr/bin/env bash
# Flatpak command for the Kylin bundle. Pick one input-method environment
# from the host session, then replace this process with Studio.
# `--print` writes the decision. A test sets KYLIN_IM_BUS_OWNERS and does
# not talk to a session bus. KYLIN_LAUNCH_TARGET overrides the binary.
# KYLIN_IM_MODULE_ROOT overrides /app/lib/im for the bundled module files.
# KYLIN_IM_BACKEND_PROBE overrides the GDK backend executable in tests.
set -euo pipefail

kylin_bus_has_owner() {
  local name="$1" reply
  if [[ -n "${KYLIN_IM_BUS_OWNERS+x}" ]]; then
    case " ${KYLIN_IM_BUS_OWNERS} " in
      *" ${name} "*) return 0 ;;
      *) return 1 ;;
    esac
  fi
  if ! command -v dbus-send >/dev/null 2>&1; then
    return 1
  fi
  reply="$(dbus-send --session --reply-timeout=1000 --dest=org.freedesktop.DBus --print-reply \
    /org/freedesktop/DBus org.freedesktop.DBus.NameHasOwner "string:${name}" 2>/dev/null || true)"
  case "$reply" in
    *"boolean true"*) return 0 ;;
    *) return 1 ;;
  esac
}

# module-id is one token. The cache file must sit beside it.
kylin_read_module_id() {
  local id_file="$1" cache id
  cache="$(dirname "$id_file")/immodules.cache"
  [[ -f "$id_file" && -f "$cache" ]] || return 1
  IFS= read -r id <"$id_file" || return 1
  id="${id//$'\r'/}"
  id="${id#"${id%%[![:space:]]*}"}"
  id="${id%"${id##*[![:space:]]}"}"
  case "$id" in
    ""|*[!A-Za-z0-9_-]*) return 1 ;;
  esac
  printf '%s' "$id"
}

# Print GTK_IM_MODULE, GTK_IM_MODULE_FILE, XMODIFIERS, DBUS, and NOTE.
# An empty value means that variable is left unset. NOTE is not exported.
kylin_select_input_method() {
  local gtk="" file="" xmod="" dbus="" note="" backend=""
  local has_fcitx5=0 fcitx_service="" has_ibus=0 module_id="" display_number
  local root="${KYLIN_IM_MODULE_ROOT:-/app/lib/im}"
  local id_file="${root}/fcitx5/module-id"
  if backend="$("${KYLIN_IM_BACKEND_PROBE:-/app/bin/kylin-im-backend}")"; then
    case "$backend" in
      x11|wayland) ;;
      *) backend=""; note="invalid-backend-probe-output" ;;
    esac
  else
    backend=""
    note="backend-probe-failed"
  fi
  if kylin_bus_has_owner org.fcitx.Fcitx5; then has_fcitx5=1; fi
  # Fcitx 4 clients address the display-specific name, not just its alias.
  display_number="${DISPLAY:-:0}"
  display_number="${display_number##*:}"
  display_number="${display_number%%.*}"
  case "$display_number" in
    ""|*[!0-9]*) ;;
    *)
      while [[ "$display_number" == 0* && "$display_number" != 0 ]]; do
        display_number="${display_number#0}"
      done
      if kylin_bus_has_owner "org.fcitx.Fcitx-${display_number}"; then
        fcitx_service="org.fcitx.Fcitx-${display_number}"
      fi
      ;;
  esac
  if [[ -z "$fcitx_service" ]] && kylin_bus_has_owner org.fcitx.Fcitx; then
    fcitx_service="org.fcitx.Fcitx"
  fi
  if kylin_bus_has_owner org.freedesktop.IBus; then has_ibus=1; fi

  if [[ "$backend" == wayland ]]; then
    if [[ "$has_fcitx5" == 1 ]]; then
      dbus="org.fcitx.Fcitx5"
    elif [[ -n "$fcitx_service" ]]; then
      dbus="$fcitx_service"
    elif [[ "$has_ibus" == 1 ]]; then
      dbus="org.freedesktop.IBus"
    fi
  elif [[ "$backend" != x11 ]]; then
    :
  elif [[ "$has_fcitx5" == 1 ]]; then
    dbus="org.fcitx.Fcitx5"
    if module_id="$(kylin_read_module_id "$id_file")"; then
      gtk="$module_id"
      file="${root}/fcitx5/immodules.cache"
    else
      note="missing-fcitx5-module"
    fi
  elif [[ -n "$fcitx_service" ]]; then
    dbus="$fcitx_service"
    if module_id="$(kylin_read_module_id "${root}/fcitx4/module-id")"; then
      gtk="$module_id"
      file="${root}/fcitx4/immodules.cache"
    else
      note="missing-fcitx4-module"
    fi
  elif [[ "$has_ibus" == 1 ]]; then
    gtk="ibus"
    dbus="org.freedesktop.IBus"
  fi

  printf 'GTK_IM_MODULE=%s\n' "$gtk"
  printf 'GTK_IM_MODULE_FILE=%s\n' "$file"
  printf 'XMODIFIERS=%s\n' "$xmod"
  printf 'DBUS=%s\n' "$dbus"
  printf 'NOTE=%s\n' "$note"
}

kylin_apply_input_method() {
  local line key value gtk="" file="" note=""
  while IFS= read -r line; do
    key="${line%%=*}"
    value="${line#*=}"
    case "$key" in
      GTK_IM_MODULE) gtk="$value" ;;
      GTK_IM_MODULE_FILE) file="$value" ;;
      NOTE) note="$value" ;;
    esac
  done <<EOF
$(kylin_select_input_method)
EOF
  unset GTK_IM_MODULE
  unset GTK_IM_MODULE_FILE
  unset XMODIFIERS
  unset QT_IM_MODULE
  # Keep the Fcitx client on Flatpak's filtered session bus, including stale address files.
  unset FCITX_DBUS_ADDRESS
  if [[ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
    export FCITX_DBUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS"
  fi
  if [[ -n "$gtk" ]]; then
    export GTK_IM_MODULE="$gtk"
  fi
  if [[ -n "$file" ]]; then
    export GTK_IM_MODULE_FILE="$file"
  fi
  if [[ -n "$note" ]]; then
    printf 'kylin-im: %s\n' "$note" >&2
  fi
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then return; fi

if [[ "${1:-}" == "--print" ]]; then
  kylin_select_input_method
  exit 0
fi

python3 /app/libexec/studio-kylin-delivery.py verify /app
kylin_apply_input_method
exec "${KYLIN_LAUNCH_TARGET:-/app/bin/Studio}" "$@"
