# Fact fixture authority alignment

The current-source fact-control group reproduced one failure: 161 passed,
while the Yolo lane case executed its tool and reached Done but presented an
empty model catalog. Its `LiveCompletion` used an explicit lane policy; the
borrowed test `SessionSurface` instead used an `AgentConfig` with neither a
permission checker nor confirmation manager. The shared production visibility
filter correctly hid that surface's tools.

Only the test fixture changed. `open_live` passes the same policy to both
components, through a small configuration parameter in the existing surface
factory. Budget fixtures retain their original guard and default settings.
The Done, original tool receipt, no-confirmation and nonempty-catalog assertions
remain. No production permission, tool filter or execution gate was relaxed.

The exact regression passed, followed by **162/162** fact-control checks.
Commands, exits and logs are in `checks.json`; independent source and log review
is in `review-runtime-evidence.json`. The original failing group and exact
reproduction remain under `real-acp/nested-shell/`.
