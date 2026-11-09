#!/bin/bash
set -euo pipefail
export PATH='/private/tmp/aster-visual-lh/bin':"$PATH"
export ASTER_CONFIG='/Users/roylin/code/aster/docs/evidence/code/2026-11-09-visual-loop-retrieval-deepseek/sr-semantic/config.retrieval.acl'
cd '/private/tmp/aster-visual-lh/sr-sem/ws'
printf '\033]0;ASTER-VIS-SR-ws-1794188653\007'
clear
exec aster --model 'deepseek/deepseek-flash' --no-plan --fullscreen
