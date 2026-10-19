#!/bin/bash
# Synthetic review cases use the prepared verifier's pinned, offline tool cache.
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/root/.local/bin
if /root/.local/bin/uvx --offline -p 3.13 -w pytest==8.4.1 -w pytest-json-ctrf==0.3.5 \
    pytest --ctrf /logs/verifier/ctrf.json /tests/test_outputs.py -rA; then
    echo 1 > /logs/verifier/reward.txt
else
    echo 0 > /logs/verifier/reward.txt
fi
