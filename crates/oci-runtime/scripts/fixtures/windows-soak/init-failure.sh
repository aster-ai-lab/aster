#!/bin/sh
set -eu

log='/mnt/rw/lifecycle.log'

{
    printf 'phase=begin\n'
    printf 'scenario=%s\n' "${ASTER_INIT_SCENARIO:-missing}"
    printf 'phase=failure\n'
    printf 'exit=42\n'
} > "$log"
exit 42
