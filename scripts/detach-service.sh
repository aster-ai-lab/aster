# Sourced by scripts/ensure-*.sh. Do not execute directly.
# Tilt runs each local_resource with setsid, then SIGKILLs that process
# group as soon as the script exits. The service has to be a different
# session, and the pid file has to be that process — not a shell wrapper.
#
# Restart order is mandatory: require_repo_script, resolve hosts, cargo
# build, then stop_service. Stopping first drops the live listener when a
# later step fails (missing helper, cargo lock, compile error). Tilt does
# not retry that update until a watched file changes.

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo "source scripts/detach-service.sh; do not execute it" >&2
  exit 1
fi

# Fail while the current listener is still up. Call this before stop_service.
require_repo_script() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "missing ${path} — refusing to stop the current process" >&2
    return 127
  fi
}

service_pid_alive() {
  [[ -f "$pid_file" ]] || return 1
  local pid
  pid="$(cat "$pid_file" 2>/dev/null || true)"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

remember_listener() {
  local port="$1"
  local listener
  listener="$(lsof -nP -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null | head -1 || true)"
  if [[ -n "$listener" ]]; then
    echo "$listener" >"$pid_file"
  fi
}

# Seconds since the process started (portable across macOS/Linux `ps -o etime`).
process_age_seconds() {
  local etime days=0 h=0 m=0 s=0
  etime="$(ps -o etime= -p "$1" 2>/dev/null | tr -d ' ')"
  [[ -n "$etime" ]] || return 1
  if [[ "$etime" == *-* ]]; then
    days="${etime%%-*}"
    etime="${etime#*-}"
  fi
  IFS=: read -r -a parts <<<"$etime"
  case "${#parts[@]}" in
    3) h="${parts[0]}" m="${parts[1]}" s="${parts[2]}" ;;
    2) m="${parts[0]}" s="${parts[1]}" ;;
    *) s="${parts[0]}" ;;
  esac
  echo $((10#$days * 86400 + 10#$h * 3600 + 10#$m * 60 + 10#$s))
}

# True when the healthy process in $pid_file predates its executable, or the
# executable predates its sources. A healthy listener alone must not pin an
# old build: Tilt reruns `ensure-*` on every source change.
service_outdated() {
  local bin="$1"
  shift
  binary_is_stale "$bin" "$@" && return 0
  service_pid_alive || return 1
  local pid age bin_mtime now
  pid="$(cat "$pid_file")"
  age="$(process_age_seconds "$pid")" || return 1
  bin_mtime="$(stat -f %m "$bin" 2>/dev/null || stat -c %Y "$bin" 2>/dev/null)" || return 1
  now="$(date +%s)"
  ((now - age < bin_mtime))
}

# Stop the process in $pid_file and wait for its port to be released.
stop_service() {
  local port="$1" pid
  service_pid_alive || return 0
  pid="$(cat "$pid_file")"
  kill "$pid" 2>/dev/null || true
  for _ in $(seq 1 40); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.25
  done
  kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    lsof -nP -tiTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1 || break
    sleep 0.25
  done
  rm -f "$pid_file"
}

# True when the executable is missing or an input file is newer.
# A warm `cargo build` still takes the target-directory lock and fingerprints
# the graph. Several services doing that at once is what makes one start slow.
binary_is_stale() {
  local bin="$1"
  shift
  [[ -x "$bin" ]] || return 0
  local newer
  # Docs and READMEs sit next to sources. They must not force a rebuild.
  newer="$(
    find "$@" \
      \( -name target -o -name .git -o -name .data -o -name .a3s -o -name node_modules -o -name dist -o -name docs \) -prune \
      -o -type f \( -name '*.rs' -o -name 'Cargo.toml' -o -name 'Cargo.lock' -o -name 'build.rs' -o -name '*.sql' -o -name '*.proto' \) \
      -newer "$bin" -print 2>/dev/null | head -1 || true
  )"
  [[ -n "$newer" ]]
}

# Background argv in a new session and record its pid in pid_file.
# Called in the foreground so $! is the child. That child execs the
# service, so the pid does not change.
launch_service() {
  if [[ $# -lt 1 ]]; then
    echo "launch_service: missing command" >&2
    return 64
  fi
  if [[ "${A3S_LAUNCH_APPEND:-}" == 1 ]]; then
    if command -v setsid >/dev/null 2>&1; then
      setsid "$@" >>"$log_file" 2>&1 </dev/null &
    else
      perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid: $!"; exec @ARGV or die "exec: $!"' -- "$@" >>"$log_file" 2>&1 </dev/null &
    fi
  elif command -v setsid >/dev/null 2>&1; then
    setsid "$@" >"$log_file" 2>&1 </dev/null &
  else
    perl -MPOSIX -e 'POSIX::setsid() >= 0 or die "setsid: $!"; exec @ARGV or die "exec: $!"' -- "$@" >"$log_file" 2>&1 </dev/null &
  fi
  echo $! >"$pid_file"
}
