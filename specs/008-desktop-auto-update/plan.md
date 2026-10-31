# Implementation Plan: Desktop auto-update from GitHub Releases

**Branch**: none (main) | **Spec**: [`specs/008-desktop-auto-update/spec.md`](spec.md) | **Input**: [`specs/008-desktop-auto-update/spec.md`](spec.md)

**Gate context**: `apps/studio` (product regime), plus one workflow and one
python check in the root repository.

## Summary

The Aster desktop updater gains a GitHub Releases feed: `update_channel`
maps the Aster identity to a new closed enum `FeedKind::GitHub`, the
manifest `desktop-latest.json` parses into the existing `ReleaseMeta`, the
download rides the providers curl pattern (config stdin, no argv
credentials), and every existing verification/replace/rollback step stays
unchanged. The renderer adds a startup-plus-24 h automatic check behind a
preference, leaving install manual. A `desktop-v*` workflow produces the
zip and the manifest, and a hermetic python check pins the manifest
contract. The ToYou gateway path is untouched.

## Constitution Check

*GATE: Re-read `.specify/memory/constitution.md`. A NON-NEGOTIABLE
violation stops the plan.*

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Every feed refusal maps to an existing named `UpdateFail` state; missing platform/digest/version regressions all refuse, never default |
| II. Evidence retained | PASS | `docs/evidence/desktop/2026-10-22-desktop-auto-update/OUTCOME.md` |
| III. Hermetic tests | PASS | Host lib tests (fixtures, no network), renderer vitest, python manifest check; the loopback e2e runs a local static server, not GitHub |
| IV. Closed sets | PASS | `FeedKind` with `parse()`/`as_str()`; manifest platform keys are a closed match on the running platform |
| V. One workspace / published APIs | PASS | No new crates; `aster-studio` stays in the root workspace, `publish = false` unchanged |
| VI. Local schema changes | N-A | No database schema |
| VII. Shrink-only ratchets | N-A | No allowlists touched |
| VIII. Replaceable transports | PASS | The GitHub transport is added beside the gateway transport behind one enum; the gateway transport is not modified |
| IX. One configuration language | PASS | The new preference key rides the existing ACL-parsed preference pipeline (`classification.json` → generate) |
| XIII. One repository | PASS | Everything lands in this repository; the manifest lives on this repository's own GitHub Releases |
| XIV. Names are not guessed | PASS | Manifest field names are pinned by the python contract check and the parser test with fixtures; the release asset name `desktop-latest.json` is the recorded decision |
| XV. Meaning is not matched | PASS | No regex/wordlist meaning matching; JSON fields are typed structurally |
| XVI. Pickers are drawn | N-A | No picker UI |
| XVII. Dialogs stay in place | N-A | About page only extends existing states |

## Project Structure

### Documentation (this feature)

```text
specs/008-desktop-auto-update/
├── spec.md
├── plan.md
├── research.md
└── tasks.md
```

### Source Code

| Layer | Path | New files |
| --- | --- | --- |
| Aster desktop (submodule) | `apps/studio/src-tauri/src/app_updater.rs` | feed branch, `FeedKind`, manifest parser, curl download |
| Aster desktop (submodule) | `apps/studio/src/renderer/` (About/auto-check) | scheduler hook + preference toggle |
| Workflow | `.github/workflows/release-aster-desktop.yml` | desktop build + manifest step |
| Root tests | `tests/desktop_channel_manifest.py` | manifest contract check |
| Evidence | `docs/evidence/desktop/2026-10-22-desktop-auto-update/` | OUTCOME.md |

## Research

- **Feed precedent**: `apps/aster/src/cli_pair_update.rs` — `OFFICIAL_RELEASES`
  + `latest.json` + digest verify is the in-repo GitHub Releases pattern;
  the desktop manifest mirrors its shape with a namespaced asset name.
- **HTTPS transport precedent**: `apps/studio/src-tauri/src/providers.rs`
  `curl_get`/`curl_config_timeout` — curl driven from config stdin so
  nothing lands in argv; the GitHub fetch reuses this exact style.
- **Replace machinery**: `app_updater.rs` `install_zip` →
  `delivery_manifest::validate` → `verify_signature` → `replace_bundle`
  (backup) → `schedule_relaunch` (restore on failure). Untouched; only
  `fetch_current`/`download_release` gain the GitHub branch.
- **Auto-check precedent**: renderer schedulers (e.g. `useTimer`,
  knowledge revalidation cadence) and the preference store
  (`data/classification.json` + `pnpm data:generate`).
- **Workflow precedent**: `release-aster-cli.yml` digest+manifest steps;
  the desktop workflow mirrors them with `just build-studio aster`.

## Verification Plan

1. `cargo test -p aster-studio --lib -- app_updater` — new cases counted.
2. Renderer vitest — auto-check state machine.
3. `python3 tests/desktop_channel_manifest.py` — manifest contract.
4. Loopback e2e: a scratch bundle + a local static file server serving a
   fixture manifest and archive; run the real check→download→replace→
   relaunch-script path against the scratch bundle; commands and counts in
   the evidence OUTCOME.
5. `cargo fmt --all -- --check` and `pnpm lint` on touched studio files.

## Phases

1. **Host feed branch** — `FeedKind`, manifest parser, curl transport,
   channel mapping, hermetic tests.
2. **Auto-check wiring** — renderer scheduler + preference + i18n.
3. **Pipeline** — desktop workflow + python manifest check.
4. **E2E + evidence** — loopback end-to-end, OUTCOME.md, spec/task
   checkboxes closed.
