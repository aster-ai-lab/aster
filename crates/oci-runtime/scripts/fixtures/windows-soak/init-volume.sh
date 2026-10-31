#!/bin/sh
set -eu

log='/mnt/rw/lifecycle.log'
marker='/.aster-oci-create-start-smoke'

on_term() {
    printf 'phase=term\n' >> "$log"
    exit 0
}
trap on_term TERM

test "${ASTER_INIT_SCENARIO:-}" = 'volume-init'
test "${ASTER_INIT_DELAY_SECONDS:-}" = '1'
test "$(pwd)" = '/'

{
    printf 'phase=begin\n'
    printf 'scenario=%s\n' "$ASTER_INIT_SCENARIO"
    printf 'cwd=%s\n' "$(pwd)"
    printf 'state_volume=rw\n'
} > "$log"
sleep "$ASTER_INIT_DELAY_SECONDS"
printf 'phase=ready\n' >> "$log"
printf 'aster-oci-create-start-user-time-v1\n' > "$marker"

while :; do
    sleep 1
done
