#!/bin/bash
set -euo pipefail
export PATH='/tmp/aster-visual-bins-fp':\"$PATH\"
export ASTER_CONFIG='/Users/roylin/code/aster/.aster/config.acl'
cd '/tmp/aster-visual-shop-open/ws'
clear
exec aster --model deepseek/deepseek-flash --no-plan --fullscreen
