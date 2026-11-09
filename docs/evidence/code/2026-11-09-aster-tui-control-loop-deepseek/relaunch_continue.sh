#!/bin/bash
set -euo pipefail
export PATH='/private/tmp/aster-visual-cl/bin':"$PATH"
export ASTER_CONFIG='/Users/roylin/code/aster/.aster/config.acl'
cd '/private/tmp/aster-visual-cl/work/ws'
printf '\033]0;ASTER-VIS-CL-ws-1794205933-REOPEN\007'
clear
exec aster --model 'deepseek/deepseek-flash' --no-plan --fullscreen --continue
