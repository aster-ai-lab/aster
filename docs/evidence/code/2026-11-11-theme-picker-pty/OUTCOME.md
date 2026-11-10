# Theme picker labels and availability

The shipped fullscreen TUI opened the theme picker in a restored DeepSeek
session with three terminal configurations. The truecolor picker showed `auto`,
`aster-night`, `aster-day`, `tokyonight`, `rosepine-moon`, `oscura-midnight` and
`terminal`. The 256-color picker showed `auto`, `aster-night`, `aster-day` and
`terminal`. The truecolor picker with the terminal theme gate off showed six
labels, excluding `terminal`. The captured viewports contain no legacy brand
label. All six launches across the two attempts admitted zero prompts.

The first run captured all three picker viewports and completed two exit checks.
The final exit check timed out while the harness waited without reading the
PTY. The overall [first result](first-result.json) remains failed. A separate
run kept the same configurations, expected labels and ten-second exit deadline
while reading the remaining terminal output during shutdown. All three exits
completed within 0.044 seconds and produced 743 to 1411 bytes during shutdown.
The recorded relay process identities were absent after both attempts.

See the [plan](plan.json), [result](result.json) and
[observations](observations.json) for the exact command and viewport digests.
The run used byte-identical copies of the source-reconciled macOS arm64 pair at
fingerprint `87f313b79042fb5ee64806794fc7b2300fbdd298`.

## Explicitly not claimed

The evidence covers the actual picker labels, feature gating and terminal color
eligibility. Theme selection, persistence, palette contrast, native pointer
interaction and other platforms need separate coverage. The replay result
applies to the recorded exit path with continuous PTY reads.
