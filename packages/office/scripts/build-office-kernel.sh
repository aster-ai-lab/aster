#!/usr/bin/env bash
set -euo pipefail
exec bash "$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../apps/work/vendor/packages/office/scripts" && pwd)/build-office-kernel.sh"
