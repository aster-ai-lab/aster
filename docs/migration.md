# Harness migration

## Scrawl evidence internalization, 2026-10-30

The `scrawl` evidence engine is internalized at
`7b184c67767a1aee6c1d44640d94d8446cd5fed1` under `crates/capture`, with its
MIT notice and upstream Rust, Swift helper and ScrawlKit tests. This is a
source integration in the existing repository and root Cargo workspace, not a
new checkout, second lockfile or separate Scrawl.app installation. Exact
provenance and the archive hash live in `docs/provenance/scrawl.md` and
`docs/source-manifest.json`.

The existing voice module now shares `cpal 0.18.2` to avoid conflicting ALSA
link versions. The official `swift-testing` package at
`48d727cc1cf4eda667c858c501495f1018f69d21` supplies the missing test-only
framework under Command Line Tools; upstream test assertions are unchanged.
These integration changes do not establish native recording, packaged
permissions, release certification or support on another operating system.
The bounded implementation and acceptance contract is
`specs/027-scrawl-evidence/`.

## Documentation atlas, 2026-10-28

The existing static website now uses SeeCode 0.6.2 for the current Code/ACP
architecture drawings and Code Hike 1.1.0 for committed-source walkthroughs.
SeeCode's original MIT skill source is retained under `apps/web/vendor/seecode`.
Its provenance file records the archive revision and SHA-256. Code Hike and
the shared high-contrast theme are pinned in the website package lock.

These dependencies run during documentation generation. The public site
continues to load one local progressive script and local assets. It has no
browser dependency on Studio source, a model service, a CDN or a diagram API.
README vectors come from the same authored diagram specifications. Historical
9.1.5 prose is preserved; 9.1.6 source walks use the explicit committed source
revision from the documentation catalog. This is not a Rust runtime or SDK
binding change. Acceptance is recorded under
`docs/evidence/web/2026-10-28-code-documentation-atlas`.

## Native Power integration, 2026-10-22

The existing `crates/power` tree is integrated into the root Cargo
workspace for the native DeepSeek optimization work in
`specs/009-power-native-deepseek-h200`. Power uses local ACL and Runtime,
and the model-owned `aster-deepseek` crate depends on local Power without
enabling an external inference backend by default. The nested Power lockfile
is reconciled into the root lockfile rather than retained as a second lock.

## Power vendored into the parent tree, 2026-10-31

`crates/power` is no longer a gitlink to `aster-ai-lab/Power`. The previously
pinned revision `cbe53bd40a4a3ba5fdb80861c11e491ab84e62be` was expanded into
ordinary files under the parent repository so checkout no longer clones that
remote. Provenance remains in `docs/source-manifest.json`. CI's Studio
checkout action only resolves the `apps/studio` gitlink.

## Local worktree scratch removed, 2026-10-31

The untracked `work/` tree held agent git worktrees and screenshot/log
scratch. Every registered worktree tip was already behind `main` (no unique
product commits to merge, aside from a disposable worktree-preservation
commit). Worktrees were removed, `work/` deleted, and `/work/` added to
`.gitignore`. Local runtime dots (`.tmp`, `.local-archive`, `.aster*`,
`.codex-worktrees`, `.pytest_cache`) were cleared the same pass. Spec-kit
surfaces under `.claude` / `.agents` / `.cursor` / `.grok` stay — they are
the SDD procedure set, not scratch.

The pinned ONNX Runtime 2.0.0-rc.10 remains aligned with BGE/OCR. Its SmallVec
2 alpha.10 pin conflicts with the optional Mistral backend's serde-saphyr
0.0.16 and saphyr-parser-bw 0.0.605 dependencies on SmallVec 2 alpha.12.
Both published sources are retained locally with their licenses and packaged
tests; private container annotations use the stable SmallVec 1.x API
(minimum 1.15.2). Parser logic, inline capacities and public APIs are preserved.
The resolver selects compatible annotate-snippets 0.12.5 without changing
the terminal's unicode-width pin. This is dependency reconciliation,
not a new product configuration format. Provenance is in
`source-manifest.json` and the native H200 evidence directory.

Existing uncommitted Cargo/BGE/Power changes were snapshotted before this
integration and remain separate from the native work. Build and test evidence
is recorded as it completes; workspace membership alone does not establish
DeepSeek model support or GPU performance.

The model owner also contains effective-weight RMSNorm and an explicit-cache
rotary tensor transform following the pinned DeepInfer contract. The rotary
source's Apache/vLLM notices are retained in
`crates/deepseek/licenses/Rotary-Apache-NOTICE.txt`. No upstream Python runtime
is imported. The resumed implementation adds an FP32 cache-frequency factory;
complete model/runtime integration and CUDA numerical qualification remain
unfinished obligations.

The resumed 2026-10-23 implementation retains the interrupted 281-test
decoder/state checkpoint and adds exact official base-text weight bindings.
All 40 layers' 93,415 declared tensor descriptors match independently audited
official headers. Small effective BF16/F32 parameters use bounded Power-owned
reads; FP8/MXFP4/Engram matrices retain explicit raw/scale layouts and row
loading. The fixture is metadata only, not product configuration. Complete
model composition, CUDA qualification and native deployment remain unfinished;
the native H200 evidence records 287 passing CPU tests and existing workspace
gate failures separately.

The next T007 slice adds raw matrix upload in independently bounded,
scale-block-aligned reads, actual official layer composition, sequential
cross-layer top-k/candidate sharing and `forward_pending` execution through
the complete backbone transaction. Model values remain on the selected
device; bounded integer selection metadata is explicit host control. A
failed layer poisons both the step plan and the uncommitted transaction.
The 302-test CPU acceptance closes the task's operator/layer/state scope;
SM90 kernels and GPU qualification remain T008, and the complete model
loop, capacity placement and H200 model validation remain T009.

T008 now adds original model-owned CUDA implementations for group-32 FP8
MMA and a separately selected raw-quantized FP32 scalar control. The latter
retains extreme checkpoint exponents without a BF16 intermediate. MXFP4
nibbles convert losslessly to E4M3 only in registers. No SM100 kernel source
was transplanted. The fragment coordinates follow CUDA 12.6 PTX ISA 8.5,
section 9.7.15.4.10. Candle 0.10.2 and its existing cudarc/NVRTC dependency
provide the stream/storage bridge; Power's F32 API and dependencies stay
unchanged. Receipts identify the selected profile, source/PTX, compile flags
and actual driver allocation attributes. Unsupported architecture, scale
envelope or workspace refuses rather than selecting another profile.
The 305 CPU tests, CUDA feature check, and SM90 assembly checks do not close
T008's actual H200 numerical and adapter qualification. No native service
or performance result is claimed from those checks.

The sparse-index tensor control and packed group-32 FP4 encoder/decoder also
remain model-owned Rust code. Scale rounding and E2M1 RNE/packing follow the
fixed TileKernels contract; candidate selection follows DeepInfer. The index
packed-scale boundary refuses zero/padding and nonfinite bytes, and does not
change the general checkpoint E8M0 decoder. DeepSelect/DeepGEMM sources were
audited without importing their kernels; GPU and complete-model proof remain
unfinished. Exact references and format distinctions are retained in the
native H200 evidence directory.

The 2026-10-18 import copies the required Rust source snapshots from
`../aster` into a standalone Aster workspace. The OS/backend checkout remains separate; the later Desktop cleanup is
recorded below. Package-level provenance is recorded in
[`source-manifest.json`](source-manifest.json). Local snapshots include
working-tree changes; repository revisions do not imply release certification.

## Product and dependency boundaries

`aster` directly runs the existing pager composition root. It starts the
sibling `aster-acp` process, which owns the Code Core harness. There is
no umbrella CLI or second agent loop. The old Aster umbrella CLI, cloud,
gateway, other desktop apps, web sites, distribution installers, and Code's
Node/Python/Go SDKs are outside this product.

The imported support crates cover the harness fact log, ACL parser,
native sandbox, memory, lexical retrieval, search/browser contracts,
extension reconciliation, OKF, optional workflows, and typed decisions.
Terminal support remains in its upstream directory structure to retain
resource paths, test fixtures, protocol definitions, and third-party notices.
The PTY controller and harness remain available for terminal regression tests.

All imported Rust packages and Work are members of one Cargo workspace with resolver 3,
shared dependency declarations, one checked-in `Cargo.lock`, one build
directory, and Rust 1.97.1. This follows the
[Cargo workspace model](https://doc.rust-lang.org/cargo/reference/workspaces.html).
The two default members build the terminal and ACP companion together.
Retained package names and Rust module paths keep the embeddable SDK stable.

## Compatibility decisions

- **Flow:** Code 9.1.4 uses Flow 1.1 contracts removed in the sibling's
  Flow 2.0 tree. The exact Code-pinned Flow commit
  `2948ad51a1395177764766c3ddf7e44338f9e374` is copied from the existing
  Cargo source cache. It is a local workspace dependency. Flow's optional
  Boot, Event, and ORM integrations now resolve to in-tree sources too.
- **Small support libraries:** Common 0.1.1, Lane 0.5.1, and Updater 0.2.0
  have no source project in the sibling tree. Their exact cached crate
  sources are included locally, with original licenses and provenance.
  Updater is retained for the standalone Search package; Aster's own
  automatic and explicit binary-update entry points are disabled.
- **ACL:** all imported consumers use the sibling's ACL 0.3.0 locally.
  Host-authoritative configuration remains protected from repository
  overrides. Workspace MCP trust and sandbox checks remain in place.
- **SQLite:** ACP and Memory move from rusqlite 0.32 to 0.37, matching
  the terminal crates, because Cargo permits only one `sqlite3` native
  linkage. Both memory adapters use sqlite-vec 0.1.7-alpha.10. Existing
  database schemas and migrations remain intact.
- **Runtime:** Tokio Util requires at least 0.7.19 for cancellation-token
  identity comparisons already used by Core.
- **Vector test utilities:** Vec's old exact tempfile 3.14 dev pin is
  widened to 3.27 to share the workspace's temporary-file library.
- **Configuration and state:** existing `.aster` / `.aster` paths and
  `ASTER_*` environment variables remain compatible. The migration copies
  no personal credentials, session stores, browser profiles, or build outputs.
- **macOS native execution:** the source Core deliberately refuses native
  shell execution with `native_process_lifetime_unqualified` until descendant
  lifetime and settlement are qualified. This guard remains intact. The two
  shell-dependent PTY success tests are skipped only on macOS; native refusal
  tests cover the guard. Launching, model chat, and file tools remain available.
- **SDK-independent fixtures:** evaluation/research wire fixtures move
  from `sdk/` into `crates/code/fixtures/protocol`; Rust protocol tests
  point at the new paths. No foreign-language bindings are retained.
- **Release tests:** Search's standalone upstream release-workflow test is
  retired with that release pipeline. Root workspace CI and Aster's
  migration tests replace the obsolete publication-layout assumptions.
- **Memory tests:** the legacy ACP `v912_remember_before_stream_close`
  oracle expects a raw-input fallback and JSON files even when the extraction
  model returns no memories. Current Core explicitly forbids that fallback
  and uses Vec snapshots. The historical test remains ignored with its reason;
  Core's current memory-retention tests verify the supported contract.
- **Hardware and linking:** retain Apple Silicon's 16 KiB jemalloc page
  calibration, Linux ARM64 page settings, static MSVC CRT settings, and
  musl linker hardening. Python-extension dynamic lookup flags are removed.

## Licensing

The Aster entry point and the repository root license are MIT. The terminal
tree retains Apache-2.0 in `crates/terminal/LICENSE`. Code Core, ACP, and the
supporting Aster libraries retain their original MIT licenses. Third-party
notices remain under `crates/terminal`. The root MIT license does not
relicense the terminal tree, vendored Apache text, or the AGPL desktop
components.

## Validation

Validation results are recorded in [`validation.md`](validation.md).
Real-model qualification, external browser installation, optional cloud
backends, and release certification are separate from the local migration.

## Local launch commands

`just code` builds both default workspace members and runs the current
`aster` TUI. Arguments are forwarded as positional shell arguments.

`just studio`、`just dev` 和兼容入口 `just work` 默认启动 AsterDream。
构建和启动均从根工作区执行；`ASTER_TENANT_SLUG=toyou` 选择通友托管版，
`ASTER_TENANT_SLUG=aster` 选择本地版，未设置时默认本地版。
`just build-studio` 先构建 ACP 和 Office，再构建 `apps/studio` 宿主。
`just build-studio-release` 打包所选产品。构建失败不会启动旧二进制。
这两个产品分别保留自己的构建身份、配置和数据目录。

The welcome screen uses the full and compact ASCII artwork retained at
`crates/terminal/crates/codegen/aster-pager/assets/logo/aster.txt` and
`crates/terminal/crates/codegen/aster-pager/assets/logo/aster-small.txt`.
Forty-eight frames are cached lazily and replayed at 12 FPS. Glyph and color
sweeps share the existing welcome-screen redraw clock. Compact artwork reserves
five rows so a standard 80-by-24 terminal can still show it. Color output
uses the terminal's existing quantization and NO_COLOR handling.


## Complete internalization

The workspace now contains 137 Rust packages. Aster dependencies in every
manifest and the lockfile resolve to local source, including optional,
target-specific, and test dependencies. Every workspace member is marked
`publish = false`. The additional source snapshots are recorded in
[`internalized-sources.json`](internalized-sources.json).

Work shares ACL, OKF, Vec, Use Core, and reconciliation with Code. OCR,
Use Extension, Tool Acquire, Boot (including macros), Event 0.3, ORM, and
Office's four native/kernel crates are included. The pinned OpenAI client
fork and Work's modified PTY adapter are local as well. OCR's image dependency
uses the workspace-compatible 0.25.10 release.

Code, Work, and Desktop share the Flow 2 engine at `crates/flow`, with
package name `aster-flow` and library identity `aster_flow`. The initial import
retained Flow 1 separately for Code at commit
`2948ad51a1395177764766c3ddf7e44338f9e374`. The root workspace now resolves
one engine, `aster-flow` 2.0.0, from `crates/flow`. `crates/flow-v2` is not a
workspace member. The workflow authoring package stays under
`crates/flow/packages/ui`.

Work's desktop copies of the shared Rust libraries and its separate lockfile
are removed. Its renderer stays in a pnpm workspace. Office's full editor
source, build configuration, and checked-in WASM kernel replace the old
artifact-only dependency; `pnpm install` builds the editor locally. Work's
source packages are private and standalone package publication scripts are
removed. Its default tenant assets and formerly external test fixture are
included. Source builds, development launchers, macOS packaging, and the
Linux container build use the root Cargo workspace and target directory.

Work remains AGPL-3.0 under `apps/work/LICENSE`; the Rust libraries and third-
party forks retain their respective original licenses.

## Desktop import

Desktop is imported from `../aster/apps/desktop`, including its local source
changes. Provenance is recorded in `source-manifest.json`. It stays at
`apps/desktop` alongside Aster Work. The host (`aster-desktop`), native database
runner, and lightweight desktop contract tests join the root Cargo workspace.
There is one Cargo lockfile and target directory; the default members stay
Aster and ACP. All three imported packages have `publish = false`.

Desktop shares local ACL 0.3, Boot, Flow 2, OKF, OCR, Vec, and Use libraries.
The duplicate ACL authoring dependency is consolidated into the local parser.
ORM gains the upstream engine catalog and statement-grid APIs. Use Extension
gains the Paperloom host mode and composer-mode contract. These additions
preserve existing public APIs. Stale Desktop memory diagnostics and tests use
the current personal/private scope contract; room-memory isolation remains.

Frontend libraries live under `packages`: UI, Intelligent UI, Form, CE, and
an independently installable copy of the internalized Office source and kernel.
Flow UI stays under `crates/flow/packages/ui`. `apps/web` contains only the
shared Flow designer, supporting components, tenant assets, and an AIX fixture.
Registry source packages under `apps/use-registry/packages` supply renderer
imports and native fixtures; no web or cloud service is imported.

Install the frontend in `apps/desktop` with `pnpm install --frozen-lockfile`.
Run `just build-desktop` at the root to build ACP and Office, then Desktop.
`just dev` starts the development window. On macOS,
`just build-desktop-release` packages the frontend, host, and companions.
Launch and packaging scripts resolve custom target directories through Cargo.
Desktop keeps its upstream package, binary, tenant, and application identifiers.
Existing user data is not copied or migrated.

Desktop retains AGPL-3.0 under `apps/desktop/LICENSE`; libraries retain their
original licenses and notices. Standalone publication scripts and release
workflows are excluded. No credentials, local environment secrets, sessions,
Git metadata, build caches, or compiled companion binaries are imported.

The Desktop renderer builds successfully. Its full TypeScript check still
reports upstream errors (324 diagnostics locally, compared with 349 in the
source checkout); every distinct local error also appears upstream. CI gates
the Desktop renderer build and launcher tests while retaining Work's existing
type-check gate. Fixing unrelated frontend typing is outside this import.

After validation, `../aster/apps/desktop`, `../aster/packages/intelligent-ui`, and
`../aster/packages/intelligent-ui-aix` are removed from the source tree and moved
to the local Trash for recovery. OS, API, Gateway, and shared UI/Office/Form/CE/
Flow sources remain there. Desktop's old submodule entry and launch commands
are removed from the OS checkout. Four backend live tests now read the identical
PDF fixture at `../aster/fixtures/paperloom/two-pages.pdf`, so removing Desktop does
not remove their input. Offline frontend installation and Rust checks pass
again with the source Desktop directory absent.

## macOS command lifetime follow-up

On 2026-10-18, Aster adopted the owner-bound Guest adapter from Code commit
`cd3764c9` and the host-availability provider from `cb5b5521`. The paired OCI
Runtime shim is source commit `3189636cb`; it watches both the direct Core
process and the shim that owns the VM worker. The adapter and shim must be
deployed together. Exclusive command-script creation also rejects existing
files and links without overwriting them.

[Retained execution evidence](evidence/execution/2026-10-18-macos-guest-owner/OUTCOME.md)
binds the exact local sources and signed fixture artifact. Native macOS Shell
remains refused; product packaging, policy composition, control-file recovery,
network mediation, background streaming and the Python environment remain open.
No additional language SDK or in-process code sandbox was introduced.

## Backend services import

A second 2026-10-18 import brings the Aster OS backend into this workspace as
five new members: `apps/api`, `apps/auth`, `apps/billing`, `apps/config`, and
`apps/cloud/crates/sdk`, cut from `../aster` at revision
`cde6ae1e0a1b0ee36078e12cf5defeeb814715d4`. Provenance and per-tree hashes are
recorded in [`source-manifest.json`](source-manifest.json) under
`backend_import`.

- Dependencies resolve locally: the four service manifests replace the
  `aster-ai-lab/Boot` and `aster-ai-lab/ORM` git pins with `crates/boot` (0.2.0) and
  `crates/orm` (0.3.1), the same versions the pins named. The Use extension
  path follows this repository's flattened `crates/use/extension` layout.
- Member `Cargo.lock` files are dropped; the root lockfile is the only one.
- Rust sources are reformatted to the workspace's rustfmt defaults. A
  whitespace-insensitive audit attributed every differing source file to that
  reformatting; the only content change is the removal of the `maas-leave`
  remote agent, deleted together with the aster-maas service in the source
  repository.
- `apps/use-registry` and `apps/web` are imported as content trees (no Cargo
  membership); caches, build outputs, and local state are excluded.
- Validation: `cargo check --tests` on the five members, workspace
  `cargo fmt --check`, and a clean-room boot of all four processes against the
  live middleware on fresh databases — `/health` fully up, admin login through
  the auth directory, and a notes round-trip. See
  [`source-manifest.json`](source-manifest.json) `backend_import.verification`.
- Inherited debt: a subset of the api's in-source test campaigns fail at the
  source revision too (one overflows its stack, one architecture ratchet is
  red); they are recorded here rather than fixed, to keep the copy faithful.

## Cloud control-plane import

The same day, the cloud dependency closure lands: `crates/box` (the sandbox
runtime workspace: core, runtime, netproxy, plus the vendored deps and
guest sources that stay unregistered), `crates/runtime`, `crates/sms`,
`crates/form-core` (from the pinned aster-ai-lab/UI source), `crates/oci-sdk`
and `crates/oci-runtime` (from the pinned aster-ai-lab/OCI-Runtime source), the
`aster-use` umbrella package, and the cloud members `apps/cloud/crates/
{contracts,node-agent}` joining the already-present sdk.

- `apps/cloud/crates/control-plane` is copied but not registered: its HEAD
  depends on the in-flight use-core lane (PluginSurfaceKind and the
  PluginSurfaceRef narrowing) that no published use-core carries yet. It
  registers once that lane lands.
- Version pins carry over as local paths: Boot/ORM/ACL/Use resolve to the
  in-tree crates; Box 3.2.7, Runtime 0.8.0, and UI form-core resolve to the
  copied trees. The box runtime keeps axum 0.6 next to the workspace's 0.8
  (its scale API predates the 0.7 rename), and use's rusqlite aligns to 0.37
  for the single native sqlite3 linkage.
- Validation: `cargo check` on contracts, node-agent, runtime, sms,
  form-core, and box-runtime, plus workspace-wide `cargo fmt --check`.

## Desktop delivery resource repair

The web application remains removed. Desktop still consumes its public tenant
brand records and the launch check's pure tenant parser. The 2026-10-21 Desktop
alignment restores only those source assets from the parent of `e07fd7c`;
the exact file hashes are in
`docs/evidence/desktop/2026-10-21-work-alignment/tenant-restored-source.json`.
The removed JSON model-market configuration is not restored. Desktop retains
its existing native provider and organization eligibility boundaries.
Packaged Desktop carries the selected tenant with its renderer and companions,
so an installed application no longer needs this checkout for branding.

## Gateway, runner, and the full-module state

`apps/gateway` registers with the bge embedding crate and the vendored
updater and sentry sources; `apps/runner` registers. The science, web,
and official content trees are no longer in this checkout. crates/vec loosens its rayon pins
from exact 1.10.0 to ^1.10 so the gateway's image/ravif chain resolves
beside the vec index pin wall. apps/runtime (the Gateway /v1 data-plane
binary) shares the aster-runtime package name with the copied exec
runtime and stays a standalone tree pending its rename decision. The
registry-builder tool ships unregistered against the pinned extension
0.1.3 for the same reason. Validation: vec 459, runner 33, gateway
compile-plus-tests, and the terminal product still compile.


## Default AsterDream desktop tenant

The current desktop product uses the shared host under `apps/desktop`.
The default `aster` build retains AsterDream's canonical star/orbit assets and
blue/silver theme. Build selection also selects local/BYOK versus managed
organization code. Installed and launch identities must match the selected build.

This consolidation imports functionality and branding only. The new `aster`
profile never opens the legacy Work profile. No sessions, projects, preferences
or credentials are migrated. `apps/work` remains legacy source and supplies the
Office vendor dependency. Root workspace package identifiers remain unchanged.

Current requirements and verification are tracked in
[`specs/004-desktop-default-aster`](../specs/004-desktop-default-aster/spec.md).
Canonical asset hashes and original source paths are retained in
`docs/evidence/desktop/2026-10-21-default-aster/brand-canonical-sources.json`.
The imported upstream README is kept under `docs/provenance`.
Licenses stay with each component.

The model-owned mHC tensor composition under `crates/deepseek/src/layers/mhc.rs`
adapts the TileKernels Torch reference at revision
`c1cf914a82e6901c679445f872051ce44ac2cc63` to Rust/Candle operations. It retains
DeepSeek's MIT notice in `crates/deepseek/licenses/TileKernels-MIT.txt`.
This is an unfused correctness path, not a GPU performance qualification or
complete model import. Checkpoint metadata and shard identities are separately
pinned and do not imply their payloads have already been verified.

The subsequent model-owned Engram slice adds exact tokenizer compression,
hash tables, image-boundary n-gram gathering/history and a tensor gate. The
normalization/gathering semantics adapt DeepInfer revision
`9bd9cf243fc53ef9afbf8f04d391d989117d6ab9`; its Apache-2.0 license is retained
in `crates/deepseek/licenses/DeepInfer-Apache-2.0.txt`. The MIT TileKernels
reference also informs Engram math and stable MoE selection/weighted SwiGLU.
Public tokenizer/hash data, complete-map agreement and retained source hashes
are documented in the model crate's fixture README. No user data, runtime
credentials, external model server, or Xolotl implementation was imported.

The pre-norm C1/C2 tensor state adapts the same DeepInfer compression reference.
It snapshots proposals and keeps only one unmatched C2 row, with accepted-prefix
commit and cancellation. The decoded sparse MQA control follows FlashMLA's
reference attention/sink math at revision
`2e5429fc5653bab6e081f09477126f731882a6a9`; its original MIT notice is retained
in `crates/deepseek/licenses/FlashMLA-MIT.txt`. These are model components,
not a complete KV cache, packed-cache CUDA implementation or deployed model.

## 2026-10-21 本地产品合并

桌面宿主统一位于 `apps/studio`。`aster` 构建保留 AsterDream 原本地版的
品牌、导航、模型配置和文件工作流，直接进入应用，不包含登录界面或用户资料展示。
本地宿主拒绝成员和组织接口，不读取成员会话；自备模型密钥只用于所配置的模型服务。
`toyou` 构建继续使用原有登录、组织模型和兼容的数据目录。构建时选择产品，
运行时不允许切换成另一个产品。

旧应用独有的 Office 文档和表格兼容修复已移至 `packages/office`，构建脚本、
测试夹具和 CI 均改用根工作区及共享资源。旧 `apps/work` 工作树已退出构建，
其工作区注册和子模块条目已移除。未提交修改通过本机忽略目录中的原子重命名保留，
未读取或复制凭据、会话、环境密钥及旧用户数据。原许可证的逐字节副本位于
`docs/provenance/asterdream-retired/legal`。来源和文件哈希见
[`work-retirement.json`](evidence/desktop/2026-10-21-default-aster/work-retirement.json)
及 [`office-relocation-source.json`](evidence/desktop/2026-10-21-default-aster/office-relocation-source.json)。

本次没有导入旧 AsterDream 的项目或会话，也没有发布新版本。新本地产品的更新渠道
尚未发布，宿主会明确返回该状态，不查询另一产品的更新源。完整验收与已知限制见
[`OUTCOME.md`](evidence/desktop/2026-10-21-default-aster/OUTCOME.md)。

## 私有桌面子仓库边界

用户授权将 `apps/studio` 改为私有子仓库，父仓库仅锁定提交引用。`apps/work` 与 `apps/desktop` 已退出目录与构建。早期完整桌面源码及源码补丁从父仓库分支和标签历史清理；原始证据在私有子仓库保留。构建仍使用根 Cargo 工作区和锁文件，CI 需配置 `STUDIO_READ_TOKEN`。

## Private submodule remotes moved to aster-ai-lab, 2026-10-31

`apps/studio`, `apps/api`, `apps/cloud`, and `apps/dashboard` now track
`git@github.com:aster-ai-lab/{studio,api,cloud,dashboard}.git`. Histories were
mirrored from the former `kense-labs` remotes (including local ahead commits on
api/cloud). The misspelled `kense-labs/dashbaord` remote is corrected to
`aster-ai-lab/dashboard`. Parent CI checkout actions and deploy-key workflows
read the new org. After cutover verification, the `kense-labs` copies were
emptied.

## 2026-10-23 原生 XQ 格式

`crates/deepseek/src/weights/xq/` 与 `xq.rs` 按固定 Xolotl revision
`b3452666e07a6374d06bb8c2185dffa271e8f65f` 实现 PTQ1_0、Q2_0、Q3_K、
Q8_0、F16/F32 的有界行解码、I32 元数据读取和固定 XQ V3 GGUF 头解析。
MIT 许可证保留于 `crates/deepseek/licenses/Xolotl-MIT.txt`，来源及哈希见
`docs/evidence/power/2026-10-22-native-deepseek-h200/xq-codec-reference-manifest.json`。
固定参考向量和真实头部描述对照不构成完整模型、CUDA dispatch、H200 性能、
访问地址或 Key 的交付证明。没有新增 SDK、语言绑定或工作区。

后续存储切片在 Power 增加模型中立的 `VerifiedRawWeightFile`：同一文件句柄
执行完整 SHA256 校验并保留分块散列，后续范围读取验证被触及的完整块。
模型所有者通过该公开边界绑定完整 XQ 身份、GGUF 行和独立 Gate/Up、Down
专家分区。`cuda/xq.cu` 为原始混合量化字节增加显式 F32 warp 数学对照，
保留上述格式来源及许可证；不常驻展开后的浮点权重，不注册完整推理后端。
CUDA 12.6 的 NVRTC/SM90 编译和 Rust adapter 检查已通过，实际 H200 数值、
完整生成、八卡效率及压力验收仍须单独完成。详细范围见实施证据。

XQ 后续组件按同一固定版本的 `ds41_dense.cpp`、`ds41_ops.hpp` 与 GGUF
导出 schema 区分官方 BF16/FP8 路径：F16 embedding/mHC 矩阵、F32 norm 与
四路 shifted mHC trunk、carried pre mix、head reduction、Q8_0 output。
新增 SM90 原始行 gather 对照，参数只通过 Power 已验证范围加载；没有将
整个权重展开为 F32 或注册完整 XQ 生成后端。运行时源码 Git blob/SHA256
清单见 `xq-runtime-reference-manifest.json`，许可证保持原样。组件 tests 和
编译结果不构成真实 H200 执行或八卡服务交付。

XQ Engram 后续切片按同版 `ds41_dense.cpp` / `ds41_engram.cpp` 核对并绑定
完整导出 U32 token map 与官方压缩词表、精确层/表长/primes/multipliers。
原始 sparse E4M3/E8M0 表仍由 Power 校验读取，F16 fused-KV 投影后执行先
归一化的 F32 门控，保留图像位与 shifted mHC carry，不套用官方 BF16 gate。
公开 token-map 小夹具、源散列及测试范围见 `xq-engram-*-audit.json` 和
`xq-engram-components-acceptance.json`；无上游 CUDA/C++ 引擎依赖或完整
原生服务的完成声明。

XQ 后续逐层切片增加独立模型/config SHA 与原始 packed kernel 要求，
从已验证 metadata 将 F32 数值绑定到 40 层及完整 53 参与者请求状态。
`layers/xq/attention/sublayer.rs` 和 `weights/xq/load.rs` 组合原始投影、
压缩/index/KV、共享选择与 FFN；失败会 poison 全部 pending state。
该接线保留同版 Xolotl 数学来源和既有 DeepInfer topology/transaction
Rust 适配的许可证。公开配置小夹具、来源及范围见
`xq-loaded-pipeline-source-audit.json` / `xq-loaded-pipeline-components-acceptance.json`。
尚未注册或交付完整原生推理服务，未将逐层硬件夹具称为整模或八卡验收。

同一固定运行时的 MoE/FFN 组合现由 `layers/xq/moe.rs` 实现：F32 路由、
原始混合 Q2_0/Q3_K/PTQ1_0 专家、Q8_0 共享分支，以及完整 shifted FFN
子层。专家行投影借用完整常驻栈，驻留字节仍由完整所有者核算；路由权重在
Down 投影后乘入，路由 lane 累加后再加共享分支，全程保留 F32 激活。
数值来源仍为清单中固定的 `ds41_dense.cpp`/`ds41_moe.cpp`，许可证不变。
组件、失败日志和编译证据见 `xq-moe-components-acceptance.json`；尚未完成
H200 数值、attention/Engram/state、完整生成、服务或性能验收。

XQ attention 后续增加原始 Q8_0 Q-A/Q-B/window/grouped-output 投影、独立
F32 RoPE 入口和保留 F16 KV 边界的 F32 sparse MQA 对照。`layers/xq/quant.rs`
按固定 `ds41_ops.hpp` 的 F32 除法实现三种激活编码，显式保留 window
FP8/group32、compressed KV FP4/E4M3/group16 和 index FP4/pow2/group32。
既有官方 BF16 编码及 RoPE/attention API 保持原合同；XQ F32 attention
拒绝已开启的全局 TF32/reduced-precision GEMM 模式，不修改进程的设置。
新组件的参数形状、整栈行借用、缓存快照及聚合预算都有明确边界；没有新增
格式猜测、绑定、SDK 或工作区。仍须完成 compressor/indexer/Engram 与事务
状态的整模接线和真实 H200/服务/性能验收。来源、范围和未验收项见
`xq-attention-components-acceptance.json`。

XQ 压缩/indexer 组件继续采用同一固定运行时：原始 F16 A/Gate/Q/K/head
矩阵与 F32 激活、C1/C2 状态、norm 和分数。`CompressionNumerics` 明确区分
默认官方 BF16 与 XQ F32，F32 接受前缀按 4 字节计费，C1 峰值和位置元数据
单独有界；事务所有权、lease、取消及原子提交保持原机制。C2 的可见位置为
pair end，RoPE 为 group start，两者分别传递；index K 使用归一化但尚未
RoPE/量化的主 latent。XQ head 权重只缩放一次 `1/sqrt(128*heads)`，top-k
前保留 F32 分数，候选末块按实际最大值加有限 `1e30`，不套用默认 BF16
profile 的数值边界。来源 Git blob/SHA256 复核见 `xq-compressor-index-source-audit.json`。
组件 CPU/构建证据不关闭真实 H200、完整 XQ Engram/backbone/model/service
及性能验收；没有导入凭据或用户状态。

2026-10-24 的原生 XQ 延续已完成真实 H200 组件资格验证，以及四个完整双卡
副本同时驻留后的固定原始 token 生成。该结果仅覆盖有界冒烟，不代表通用
文本质量或性能验收。新增进程 CUDA ordinal 与模型本地 rank 的显式映射，
以及保持独立状态所有者、有界队列和取消的完整副本池；Power chat Backend
通过类型化 composition 注入，默认引擎关闭。服务入口与限时凭据仍在验收。
文本提示编码改编自官方 `encoding/encoding.py`，revision
`1bae0e1088993b9b82de08f59de5be5a34e42170`，SHA-256
`13231c592e958fb39e2af6001dae69d90e7a36b265702b9b390236fb387599e2`；
保留 `crates/deepseek/licenses/DeepSeek-MIT.txt`，24 组独立官方编码器对照数据
位于 `xq-text-prompts-golden.json`。当前仅支持明确的文本角色，工具、图像、
跨请求缓存和长上下文不由这组对照宣称通过。

2026-10-24 的原生 XQ 实例进一步通过实际 Windows 客户端的鉴权、文本与
流式冒烟。新 CUDA Sinkhorn 是 Aster 原创的固定 4x4/20 迭代实现，保留
XQ 的 FP32、epsilon 与归一化轴顺序；通过显式构建 feature 作为实验候选，
原有 tensor control 保留。真实 H200 的独立标量与组件数值资格已通过，
微内核提速不代表完整模型收益。另一个有限值标志融合候选仍待 H200 资格。
用户明确的最终门槛为单个请求 1000 token/s，当前基本服务及短基线不能
关闭该目标。来源、构建、逐项结果和未声明范围见原生部署 OUTCOME。

后续有限值融合通过 H200 全套组件验证；短单请求端到端实测有改善，
但冷暖差异与历史基线顺序尚未排除，不将全部差值归因于融合。新增原生
单调时钟按实际生成 ID、整模步和副本记录 decode 边界，EOS 口径单列。
该 Rust 计时和 Nsight 实验 operator 为 Aster 原创，未复制外部源码；
指标放入 Power 已有受隐私开关保护的 `timings` 信封。中间 v25/v26
隐私布局在部署前被否决并保留记录。单请求 1000 token/s 仍未通过。

原生 XQ 的 v29 实验增加 Aster 原创、默认关闭的单模型步不可变 TensorId
校验复用。首次有限/非负校验仍在原边界同步执行，失败阻止事务提交；
固定容量元数据计入预算，离开模型步立即销毁，不保留 tensor/GPU 存储。
范围仅覆盖不接受外部 tensor、无原地写入的私有原生执行图。Windows 449
控制测试及 H200 451 控制/19 次 GPU 调用通过。重复校验微测试约快 29–33 倍，
整模收益尚待四副本预热后的同源码 v29/v30 对照；不据此声称达到性能门槛。

随后新增默认关闭的 Aster 原创 SM90 E4M3/E2M1 编解码融合实验，保留原 tensor
对照、RNE 中点、负零、原 F32 归一化除法及拥有者预算/错误边界。450 项本地
控制测试通过；初稿运行时编译头文件问题及修复记录均保留，修正源码已通过
严格 NVRTC 编译。同源码 v33/v34 已构建并暂存，真实 GPU 数值及整模收益仍待验收。


2026-10-24 的原生实验另保留默认关闭的只读 CUDA 输入视图复用；拥有者权重/KV 快照仍复制，原同步校验边界不变。452 项本地控制、20 项 operator、六项依赖守卫及同源码 v35/v36 Linux 构建通过，GPU 数值与整模收益未验证。用户另行要求的原版 Xolotl 试跑与原生实现分开记录，原生部署目标保持未完成。


2026-10-24 按用户要求增补原版 Xolotl 单 H200 试跑，与 Rust 原生执行分开。外部源码仅在隔离部署工件中构建，未导入产品；MIT 通知、上游/ggml 提交、Aster 默认关闭的有界图元数据与保留读回 fence 的异步实验补丁及实机对照均已留证。当前用户门槛改为单 H200 单请求至少 500 token/s，尚未完成；没有凭据或用户会话导入。

2026-10-30：Office 的 EmbedPDF 2.14.3 文本焦点修复从上游
`embedpdf/embed-pdf-viewer` 的 `v2.14.3`、提交
`c91236dab54ec1998fa2b13cf5cc43f60567ac4b` 重建。保留原 MIT 许可、版本和
公开声明，仅修复 FreeText / CalloutFreeText 的重复编辑、原生光标和空撤销记录。
原始源码补丁、生成后的 snippet 补丁及重建输入均留证；根 Bun 和 Studio pnpm
均注册并验证实际补丁文件，没有手改生成运行时，也没有导入用户状态或凭据。
后续资料库 UI 修正保留 Word 蓝、Excel 绿和 PowerPoint 橙，以文件标签页负责
切换与关闭，删除编辑区重复返回栏；菜单改为非模态，并修复共享选择器圆角变量。
验收范围及未完成项见 `docs/evidence/office/2026-10-30-library-chrome/OUTCOME.md`。

## 2026-10-25 — single-GPU DeepSeek DSpark and managed readiness

Power main 24b364c6935c1b62a689d91839ec394bfc74c059 retains the published H200/storage history and merges upstream typed WorkBuddy message metadata support. Native DeepSeek serving on GPU6 uses ACL-owned launch parameters, the pinned upstream Xolotl engine with retained license/provenance and actual DSpark draft accounting, and shared OpenAI tool compatibility. The Gateway normalizes only the enumerated client annotations. A bounded readiness wait fixes false 503 responses when concurrent health probes share the lifecycle lock. Sources and deployment evidence: tools/model-runtime/deepseek-v41-xq and docs/evidence/deployment/2026-10-25-deepseek-dspark-main/OUTCOME.md. No credentials, tenant state, weights, or unrelated concurrent Cloud/API edits are imported.

## 2026-10-25 — unified Gateway availability and neural Auto

Auto now uses the existing typed decision plane with an explicit ACL decision protocol, bounded neural Prism calls, per-turn coalescing, policy-bound frozen decisions and a primary-model floor for oversized observations. A failed console credential fetch rejects activation and retains the previous valid runtime; an authoritative empty credential list still revokes keyed routes. All platform supplier protocols share the console-key boundary. Shared message normalization includes the known WorkBuddy queue annotations without changing tool semantics. The model-service API registers the six-backend Prism pool supplier, and the existing managed snapshot publisher persists its ACL policy. Validation and retained counterexamples: docs/evidence/gateway/2026-10-25-neural-auto-readiness/OUTCOME.md. No private ACL, credentials, actual user prompts or unrelated control-plane edits are imported.

## 2026-10-28 — source-built Windows Aster TUI and Code 9.1.6

Aster 0.1.4 and its local ACP/Core 9.1.6 sources remain one paired build. The
public Code main checked for this integration is cfaa22e8f1fca7ad1bfe132ac97ce7be930071f2
(9.1.4); it does not replace the newer maintained Aster sources. Windows now
uses handle-anchored, no-follow, bounded file and directory observations rather
than the non-Unix unavailable stubs. Literal PowerShell `Get-Item` checks with
terminating errors can bind actual file bytes to completion verification.
Public Rust APIs, workspace ownership and sandbox policy are unchanged.

The native PTY acceptance suite drives the actual TUI and sibling ACP with the
operator-selected `kimi/k3.1-pro` ACL, preserving that file byte-for-byte. Its
checks include actual file bytes, committed receipts, parent-session identity,
UI state, persisted settings and independent checker execution. Evidence and
retained counterexamples live in
`docs/evidence/tui/2026-10-28-kimi-k31-source-integration/OUTCOME.md`.

## 2026-10-28 — native Linux TUI qualification in WSL

The paired Aster 0.1.4 and Code Core/ACP 9.1.6 sources also build as native
`x86_64-unknown-linux-gnu` ELF executables under Ubuntu 24.04 / WSL2. The same
ten live acceptance entry points now include Unix shell versions of the
interruption and two-turn editing fixtures. PTY input waits for rendered and
cleared composer state before submission. Frozen model prompts and byte,
receipt, session and independent-checker assertions remain in force.
The original Kimi ACL is used without importing credentials or user state.
Build identities, complete results, isolated reruns and retained input-driver
counterexamples are in
`docs/evidence/tui/2026-10-28-wsl-linux-k31-source-integration/OUTCOME.md`.

## 2026-10-29: Documentation diagrams use Pele

Replaced all 23 current SeeCode diagrams with authored Mermaid sources rendered
by the existing pinned Pele 1.10.0 dependency. Six README SVG exports now share
those sources. Preserved diagram node identities, labels, relationship labels,
ordered explanations and exact source references. Conceptual layers are block
diagrams; their arrangement does not introduce execution edges.

Removed the unused SeeCode vendor tree, project settings, motion selectors and
watermark assets. Historical provenance and evidence remain in this repository.
The Code Hike source walks and their pinned source revision are unchanged.
See `docs/evidence/web/2026-10-29-pele-diagram-migration/OUTCOME.md`.

## 2026-10-29: Spotlight documentation reading surfaces

Reworked the source walks around the official Code Hike Spotlight pattern.
Rendering uses the installed Code Hike 1.1.0 Pre and focus handlers. The existing
single navigation script now bundles Code Hike token transitions and diff using
the repository's existing esbuild 0.25.12 tool. License texts are retained in
`apps/web/licenses` and in the generated script banner.

Kept seven walks, 38 exact Git excerpts, their source revision and original line
anchors. Explicit selection replaces scroll-driven switching. Code and diagrams
share the Aster light reading surface, accessible controls and expanded reading.
Pele node identities and relationships are preserved; layout direction and
conceptual block columns change to fit the documentation canvas.

See `docs/evidence/web/2026-10-29-spotlight-reading-experience/OUTCOME.md` for actual
browser screenshots, source comparisons, reduced-motion and no-script checks.

## 2026-10-29 — native Windows Unicode text commits

Vendored the published Crossterm 0.28.1 archive under its original MIT license
and package/API identities. A narrow Windows input-parser patch treats a
nonzero UTF-16 Alt-code commit on VK_MENU release as text input. Native ConPTY
records demonstrated that the old classification dropped every combining acute
accent in the frozen long-paste acceptance case. Ordinary key releases remain
releases; surrogate-pair behavior is retained. The source archive, revision and
license location are recorded in docs/source-manifest.json.

The package joins the existing root workspace, uses its lockfile and remains
unpublished. The paired CLI source stamp now covers the local input parser.
Evidence, the failing regression and subsequent qualification are retained in
docs/evidence/tui/2026-10-28-full-acceptance/OUTCOME.md. No credentials or user
session state are imported.

## Control-plane edge workspace (2026-10-31)

Edge services `auth`, `billing`, `config`, `runtime`, and `runner` moved under
`apps/control-plane` with their own Cargo workspace and lockfile. They are no
longer root workspace members. Compat symlinks preserve the legacy `apps/auth`
family of paths for Tilt, scripts, and submodule references. `apps/gateway`
stays in the root workspace because Studio embeds it.
