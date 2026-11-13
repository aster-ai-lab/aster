set -euo pipefail
export PATH=/cargo/bin:$PATH CARGO_HOME=/cargo RUSTUP_HOME=/rustup RUSTUP_TOOLCHAIN=1.97.1-aarch64-unknown-linux-gnu CARGO_TARGET_DIR=/target CARGO_BUILD_JOBS=1
unset GROK_VERSION
export GROK_COMMIT=6a8bf40f3d10101165e18cf0f60fbd17e6213f81
cd /src
git config --global --add safe.directory /src
rustc --version
cargo --version
printf 'KRUN_BUILD_PHASE=child-tool-tests\n'
cargo test --locked --offline --profile release-dist -p aster-core --no-default-features --features local-code --lib test_stream_settles_delegated_tool_calls_before_the_parent -- --nocapture > /out/child-tool-tests.stdout 2> /out/child-tool-tests.stderr
python3 -c "from pathlib import Path; assert 'test result: ok. 1 passed; 0 failed' in Path('/out/child-tool-tests.stdout').read_text()"
cargo test --locked --offline --profile release-dist -p aster-core --no-default-features --features local-code --lib test_stream_bridges_subagent_lifecycle_events -- --nocapture > /out/child-lifecycle-tests.stdout 2> /out/child-lifecycle-tests.stderr
printf 'KRUN_BUILD_PHASE=clippy\n'
cargo clippy --locked --offline -p aster-core --no-default-features --features local-code --all-targets --no-deps > /out/core-clippy.stdout 2> /out/core-clippy.stderr
cargo clippy --locked --offline -p aster-acp --no-default-features --features svg-png,account-import --all-targets --no-deps > /out/acp-clippy.stdout 2> /out/acp-clippy.stderr
printf 'KRUN_BUILD_PHASE=cli\n'
cargo build --locked --offline --profile release-dist -p aster
printf 'KRUN_BUILD_PHASE=acp\n'
cargo build --locked --offline --profile release-dist -p aster-acp --no-default-features --features svg-png,account-import
printf 'KRUN_BUILD_PHASE=archive\n'
bash scripts/package-aster-cli.sh linux-arm64 /target/release-dist /out/aster-cli-linux-arm64.tar.gz
cargo fmt -p aster-core -p aster-acp -- --check > /out/fmt.stdout 2> /out/fmt.stderr
printf 'KRUN_BUILD_PHASE=done\n'
