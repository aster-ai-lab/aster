# Aster CLI pair update

`aster update` replaces the invoked `aster` and the sibling `a3s-code-acp` with the pinned Aster CLI release `aster-cli-v0.1.1` for this operating system. The published archive contains those two programs, and they do not implement `--build-id`. When the archive also contains both `BUILD_ID` and `SHA256SUMS`, both programs must print the same `aster-cli-pair-v1:` identity or the previous two files stay in place. A failed download also leaves the previous two files in place. The command refuses an A3S or Grok download URL. The update command itself does not write `~/.a3s` or `~/.grok`.

On a terminal, and only when the output is not JSON, stderr plays the ascii.rest `progress-bar` layout: 48 columns, 11 rows, and the five labels `fetching`, `unpacking`, `indexing`, `building`, and `linking`. The filled portion follows bytes received. The cursor is restored when the update ends. The animation does not wait for a key. `--check` and any `--json` status print no frames and do not replace files. An already-identical pair is left byte-for-byte unchanged and the text does not say `installed`.

`should_check_for_updates` still returns false. A normal launch, a leader, and stdio do not install an update. There is no `/update` slash command.

## Commands and counts

Rust 1.97.1, macOS, debug test profile. The three update tests were run twice by the already-built harness `target/debug/deps/cli_update-0850b765e54808c0 --exact --test-threads=1 --nocapture`. Each invocation exited 0.

```
target/debug/deps/cli_update-0850b765e54808c0 shipped_update_installs_newer_pair_with_progress_bar --exact --test-threads=1 --nocapture
```

1 passed, 0 failed, 5.07s, then 1 passed, 0 failed, 4.09s. Both runs replaced only the two fixture binaries, printed `installed Aster v0.1.1`, and showed the five progress-bar labels with a growing `=` fill. The isolated home snapshot was unchanged.

```
target/debug/deps/cli_update-0850b765e54808c0 shipped_update_check_and_current_leave_bytes --exact --test-threads=1 --nocapture
```

1 passed, 0 failed, 3.24s, then 1 passed, 0 failed, 3.54s. `--check --json` and `--json` reported `updateAvailable` true against a newer local fixture and `autoUpdate` false, with no progress-bar frames, and left the old bytes in place. The already-current run printed `Aster CLI pair is current` and did not print `installed`.

```
target/debug/deps/cli_update-0850b765e54808c0 shipped_update_failure_leaves_the_previous_pair --exact --test-threads=1 --nocapture
```

1 passed, 0 failed, 2.74s, then 1 passed, 0 failed, 2.65s. A truncated archive printed `release archive is incomplete`. A pair whose build identities differ printed `paired binary source identity mismatch`. Both left the previous binaries runnable at the old identity.

The same three tests also passed once earlier in this session via `cargo test -p aster --test cli_update --offline`: 3 passed, 0 failed, 19.34s.

The pinned `aster-cli-v0.1.1-macos.tar.gz` archive members are `aster` and `a3s-code-acp`. That `aster` exits 2 for `--build-id` with `unexpected argument '--build-id'`. That `a3s-code-acp` exits 1 with `unknown argument: --build-id`. `aster update` installs that two-file shape. Pair-identity rollback still applies when both `BUILD_ID` and `SHA256SUMS` are in the archive.

```
cargo test -p aster --test cli_update --offline --locked -- --test-threads=1 --nocapture
```

4 passed, 0 failed, 23.77s. Exit 0. This run used the shipped `aster update` command for install, `--check`/`--json`, the already-current pair, the truncated archive, the mismatched pair, and a two-file archive whose binaries reject `--build-id`. The two-file run printed `installed Aster v0.1.1`, did not print `identity=`, replaced both files with those binaries, and both replaced files still ran.

The same two-file test was run again on the already-built harness:

```
target/debug/deps/cli_update-0850b765e54808c0 shipped_update_installs_two_file_release_without_build_id --exact --test-threads=1 --nocapture
```

1 passed, 0 failed, 1.32s. Exit 0. The archive listing was only `aster` and `a3s-code-acp`. After install, `aster --build-id` exited 2 with `unexpected argument '--build-id'`, and `a3s-code-acp --build-id` exited 1 with `unknown argument: --build-id`. Both files with no arguments still exited 0.

The shipped debug `aster update` was then pointed at a local directory that already held the official `aster-cli-v0.1.1-macos.tar.gz`. `ASTER_RELEASE_BASE` was that directory, so this updater process did not contact GitHub. The install directory began as a forwarder to the debug binary. The command exited 0. Stdout was `installed Aster v0.1.1 into <install dir>`, then the two installed paths, and it did not print `identity=`. Stderr was empty because both streams were pipes, so the progress bar did not play. Installed `aster` is 130591552 bytes, SHA-256 `152e7967d54f2d1d153379cdd5189881c88f85492a98c3d7e9c904990d3792c4`. Installed `a3s-code-acp` is 40295024 bytes, SHA-256 `cd84e7f77ab0c5bf9797550c808b4dc58d1991703eb7b4437c10c00931ace37c`. Both hashes match the two members of that archive. `aster version --json` exited 0 and printed `{"product":"Aster","currentVersion":"0.1.1 (93a852e75ca9)","channel":"unknown"}`. `aster --build-id` exited 2 and its stderr contains `unexpected argument '--build-id'`. `a3s-code-acp` with no arguments exited 0. `a3s-code-acp --build-id` exited 1 and its stderr is `Error: unknown argument: --build-id`. The archive member cap stayed `512 * 1024 * 1024`. The install directory listing is only those two files. Marker files in the isolated home stayed `keep-a3s` and `keep-grok`. After the installed official `aster version` and `a3s-code-acp` processes ran, that isolated home also contained `~/.a3s/docs/user-guide/*.md`, `~/.a3s/config.acl`, and an ACP log. Those writes come from the published v0.1.1 binaries. The current-source updater returns before user-guide extraction. The developer home was not the `HOME` of this run.

```
cargo test -p aster --test cli_p0 --offline --locked -- --test-threads=1 --nocapture
```

`aster` and `a3s-code-acp` were built together and both printed `aster-cli-pair-v1:d781e2910c1dd3e384e0747b42e6e84cca1e0825:aarch64-apple-darwin`. The `update-check` oracle printed `updateAvailable` false and `autoUpdate` false, and the test's binary-length assertion passed. The whole `shipped_cli_p0_oracles` test then failed at `models` with `Method not found: "x.ai/models/list"` and exited 101 in 13.51s. `crates/code/acp` answers only its closed host-method set, and that set does not include `x.ai/models/list`. This diff does not change that handler or the `models` command.

## Explicitly not claimed

The hermetic two-file test uses bash stubs that reject `--build-id` the same way as the published Mach-Os. The real Mach-O pair was installed by the separate local-archive run above. `scripts/install.sh` still requires `BUILD_ID` and `SHA256SUMS` and was not changed. Hermetic tests do not download from GitHub. The official-archive run read an already-downloaded local copy and the updater process itself did not contact GitHub. No new release, and no change to the release asset pipeline. No Linux or Windows run of these tests. No `cargo build --release`. Background auto-update stays off. No `/update` slash command. `apps/cloud` was not edited. `--check` and `--json` do not contact GitHub; without a local `ASTER_RELEASE_BASE` directory they report `updateAvailable` false and `latestVersion` null. The two renames that publish the pair are not crash-atomic. The progress-bar fill is driven by received bytes. It uses the ascii.rest 0.4.0 piece's five styles and 48×11 layout (`src/pieces/progress-bar.ts`, published as `ascii.rest@0.4.0`). It does not run that piece's autonomous clock. A later `shipped_update_installs_newer_pair_with_progress_bar` run exited 0 in 40.95s and contained that piece's exact 0% and 100% frames. Every captured frame in that run matched the published style functions at one shared fill. The full `cli_p0` process is not green because `aster models` asks the ACP companion for a method it does not implement. The archive member cap was not raised.
