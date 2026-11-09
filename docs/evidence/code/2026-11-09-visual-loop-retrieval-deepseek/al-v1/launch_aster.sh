#!/bin/bash
set -euo pipefail
export PATH='/private/tmp/aster-visual-lh/bin':"$PATH"
export ASTER_CONFIG='/Users/roylin/code/aster/.aster/config.acl'
cd '/private/tmp/aster-visual-lh/al-v1/ws'
printf '\033]0;ASTER-VIS-ALV1-ws-1794188563\007'
clear
exec aster --model 'deepseek/deepseek-flash' --no-plan --fullscreen
