# Frozen-source paired build v3

All requested build steps completed successfully from the root workspace with `RUSTUP_TOOLCHAIN=1.97.1` and `CARGO_NET_OFFLINE=true`. No Rust source, candidate snapshot, previous `final-build` evidence, model submission, credential, or user session state was changed. No model, utility VM, desktop application, CUA session, or Work live qualification case was started.

| Step | Exit | Wall time |
| --- | ---: | ---: |
| Core clippy, lib/tests, locked/offline | 0 | 68.069 s |
| Scoped rustfmt check, all 13 owning files and fixtures | 0 | 0.072 s |
| Aster + ACP + Office build, locked/offline | 0 | 78.830 s |
| `just build-work` | 0 | 59.313 s |
| Work compiler-artifact query | 0 | 5.245 s |
| Core `advanced-harness` real-LLM test, `--no-run` | 0 | 32.840 s |

Exact commands, exit codes, timestamps, environment overrides, raw stdout/stderr paths, compiler target/features/profile metadata, executable paths, and full SHA-256 values are retained in `clippy.json`, `scoped-format.json`, and `builds.json`.

Clippy emitted 14 warnings. Their `(level, code, message, primary files)` multiset exactly matches the earlier `ptc/visibility-clippy.json` baseline. No new diagnostic signature was found; full current primary spans are retained in `clippy-analysis.json`. No warning suppression or source fix was applied. The known whole-workspace formatting differences were not modified.

The Work query used exactly the `just build-work` Work Cargo invocation, with only `--message-format=json-render-diagnostics` appended. It returned `fresh=false` for the Work library and binary; this is recorded as an actual build and is not described as a cache hit. No retry was made to obtain a different freshness result.

## Source and static-input integrity

- Core candidate: 2,824 files; manifest SHA-256 `af729aa0608ccd912f24e3182dbfaa2036716defadb1f9ee0acb75aa3a1638c8`.
- Paired native candidate: 7,558 files; manifest SHA-256 `981020a35d39c230eb89a2d02d1d5c6f7609083fab72eca880a9ba9bad31b8ec`.
- Both candidates match before and after all builds: every content hash and complete `rg --files --hidden` package-closure file set is unchanged; no extra/missing paths or symlink mismatch occurred. Root build inputs were also checked. See `source-before.json` and `source-after.json`.
- Additional Work inputs `apps/work/logo.png` and `apps/work/build/icon.icns` have identical before/after hashes. See `work-static-before.json` and `work-static-after.json`. The original candidate snapshots were not expanded or rewritten.

## Executable identity

Paths below come from Cargo `compiler-artifact.executable` records, rather than filename timestamps. Hashes were read from the final files after all build steps. `just build-work` may rebuild companions at their existing executable paths; consequently the final hashes are final-file observations, not claims about the bytes at the earlier paired-build step.

| Target | Final path |
| --- | --- |
| Aster | `/Users/roylin/code/aster/target/debug/aster` |
| ACP | `/Users/roylin/code/aster/target/debug/a3s-code-acp` |
| Office | `/Users/roylin/code/aster/target/debug/a3s-office` |
| Work | `/Users/roylin/code/aster/target/debug/aster-dream` |
| Core matrix test | `/Users/roylin/code/aster/target/debug/deps/test_extensibility_real_llm-d4219b08e7cc2f51` |

The Core matrix test executable has SHA-256 `dafcdb08898bbb196b05d4da739229abb81b16a2a9c60382a889e1291fa6b6dd`. All five full identities are in `builds.json` under `final_executables`.

The build queue has been released. Model-runner freezing, actual app staging/signing, renderer identity, and live qualification belong to the subsequent owning steps. These compilation results do not establish model task quality, end-to-end latency, GUI behavior, or cross-platform runtime qualification.
