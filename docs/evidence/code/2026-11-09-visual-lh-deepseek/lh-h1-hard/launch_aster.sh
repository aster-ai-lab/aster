#!/bin/bash
set -euo pipefail
export PATH='/private/tmp/aster-visual-lh/bin':"$PATH"
export ASTER_CONFIG='/Users/roylin/code/aster/.aster/config.acl'
cd '/private/tmp/aster-visual-lh/lh-h1-hard/ws'
printf '\033]0;ASTER-VIS-LH1H-ws-1794187352\007'
clear
exec aster --model 'deepseek/deepseek-flash' --no-plan --fullscreen
