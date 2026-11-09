#!/bin/bash
set -euo pipefail
export PATH='/private/tmp/aster-visual-lh/bin':"$PATH"
export ASTER_CONFIG='/Users/roylin/code/aster/.aster/config.acl'
cd '/private/tmp/aster-visual-lh/lh-h3/ws'
printf '\033]0;ASTER-VIS-LH3-ws-1794186447\007'
clear
exec aster --model 'deepseek/deepseek-flash' --no-plan --fullscreen
