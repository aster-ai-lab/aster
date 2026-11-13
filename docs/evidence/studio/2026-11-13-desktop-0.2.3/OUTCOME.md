# Aster Studio 0.2.3 发布准备（未触发发布）

> 状态：**准备完成，等待 Lyon 最后一句「发」**。`release-aster-desktop.yml` **未触发**，没有构建发布包、没有上传任何 release 资产。日期 2026-11-13。

## 背景与线上基线

| 项 | 值 |
| --- | --- |
| `desktop-stable` manifest 当前版本 | **0.2.2**（`gh release download desktop-stable --pattern desktop-latest.json` 读得） |
| 最近正式 release | `Aster v0.2.2 · macOS Apple Silicon`，2026-11-10 |
| 本次目标版本 | **0.2.3** |
| 防倒退规则 | `scripts/desktop_release_manifest.py` 拒绝 `<=` 现有版本；`0.2.3` 合规 |

## 发布机制（只读调研）

`.github/workflows/release-aster-desktop.yml` 是 `workflow_dispatch` 手动触发，必填 `version` / `notes_en` / `notes_zh_cn`，可选 `defer_runtime_components`（默认 false）。`build` job 在 `macos-14`，`publish` job 在 `ubuntu-latest`：下载 `desktop-latest.json` 做回归检查、生成新 manifest、把 `Aster-darwin-aarch64-v<version>.zip` 与 manifest 以 `--clobber` 上传到 `desktop-stable` 滚动 release。需要 `secrets.STUDIO_READ_TOKEN`。

`gh auth status` 已确认登录 `ZhiXiao-Lin`，scopes 含 `repo` / `workflow`，具备触发权限。

## 本次准备的命令与结果

### 1. 版本同步（4 处）

| 文件 | 改动 |
| --- | --- |
| `apps/studio/package.json` | `"version": "0.2.2"` → `"0.2.3"` |
| `apps/studio/src-tauri/Cargo.toml` | `version = "0.2.2"` → `"0.2.3"` |
| `apps/studio/src-tauri/tauri.conf.json` | `"version": "0.2.2"` → `"0.2.3"` |
| `Cargo.lock`（父仓，`aster-studio` 条目） | `0.2.2` → `0.2.3`，由 `cargo metadata --offline --format-version 1` 自动重生成，**未手改** |

刻意不动：`apps/studio/pnpm-lock.yaml` 与 `package.json` 中 `reflect-metadata: 0.2.2`（无关依赖版本）。

```
cargo metadata --locked --offline --no-deps --format-version 1   -> EXIT 0
git -C apps/studio diff --check                                  -> OK
```

子模块提交 `e7afbf20c9`（3 文件，+3/-3）已推送到 `studio.git main`；`just studio-land-check` → **EXIT 0**（worktree clean、delivery commits clean）。

### 2. Release notes

`notes-en.md` / `notes-zh-cn.md` 按 workflow 要求的三段式起草（New Features / Improvements / Bug Fixes；新功能 / 改进 / 错误修复），内容如实：本次**无新功能**，唯一的对外变更是 `Intl.Locale` week-info 兼容修复及其带来的 8 条 renderer 断言转绿。

用发布脚本自身校验（不是人工目测）：

```
release_notes() + validate_note_languages()  -> LANG markers OK
六个段落标题全部 OK；parse_version('0.2.3') = (0, 2, 3)
```

### 3. 落地

- 父仓 gitlink `57e57d1a94 → e7afbf20c9`，`just studio-land-check` 再次 EXIT 0。
- 父仓提交含 `Cargo.lock`、gitlink、notes 与本记录；已推送到 `origin/main`（CI 从远程 main 构建，版本改动必须在远程）。

## 待执行（需 Lyon 明确确认）

```bash
gh workflow run release-aster-desktop.yml --ref main \
  -f version=0.2.3 \
  -f notes_en="$(cat docs/evidence/studio/2026-11-13-desktop-0.2.3/notes-en.md)" \
  -f notes_zh_cn="$(cat docs/evidence/studio/2026-11-13-desktop-0.2.3/notes-zh-cn.md)"
```

触发后应核对 run 状态、`Aster-darwin-aarch64-v0.2.3.zip` 是否上传、`desktop-latest.json` 的 `version` 是否为 `0.2.3`、`history` 是否严格降序。

## Explicitly not claimed

- **未触发** `release-aster-desktop.yml`，**未构建**发布用的 `.zip`，**未上传/覆盖** `desktop-stable` 的任何资产，未创建新 tag 或 GitHub Release。
- 未验证 `STUDIO_READ_TOKEN` 是否有效（只能在 CI 运行时验证），未验证 `macos-14` runner 上的构建结果。
- 未做真窗口 GUI E2E（仍被系统级 `/bin/ps` 阻塞，见 `2026-11-12-business-function-coverage/OUTCOME.md` 第四十一、四十二轮）。
- **F 层业务功能覆盖率仍为 1/17（用户口径）/ 2/17（PLAN §9 原口径）**，发版不改变该数字。
- 本次唯一对外产品变更是 `src/renderer/utils/time.ts` 的兼容修复（+7/-3）；其余均为测试、证据与版本元数据。
