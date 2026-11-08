# native 构建失败：`agent/sandbox` 资源路径不存在（已修复）

## 1. 用户报错

`just dev` / `just studio` → `apps/studio/scripts/studio-dev.mjs:382` 的 `cargo build --locked -p aster-studio ... --bin Studio` 退出 101，前置输出：

```
resource path `/Users/roylin/code/aster/target/studio/aster/debug/studio-resources/agent/sandbox` doesn't exist
warning: build failed, waiting for other jobs to finish...
error: recipe `studio` failed with exit code 1
```

## 2. 根因：资源声明端与生成端的条件不一致

构建期的 Tauri 配置不是直接读 `tauri.conf.json`，而是由 `justfile` 的 `build-studio` recipe 通过
`python3 apps/studio/scripts/studio_product.py aster tauri-config --target-dir ...` 动态生成并写进 `TAURI_CONFIG`。

- **声明端**（`studio_product.py` `tauri-config` 分支）：只要平台是 macOS + arm64 且未设 `ASTER_DEFER_RUNTIME_COMPONENTS=1`，就**无条件**往 `bundle.resources` 加
  `<target>/debug/studio-resources/agent/sandbox/` → `agent/sandbox/`。
- **生成端**（同脚本 `stage-companions` 分支）：只有在 `crates/oci-runtime/crates/cli/Cargo.toml` 存在、且非 defer、macOS arm64 时才调用 `sandbox_assets.stage()` 创建该目录。

OCI runtime 已随「OS 级 confinement 迁到外部 kbox」被移出当前 checkout（`crates/oci-runtime/crates/cli/Cargo.toml` 实测缺失，`crates/` 下已无该目录）。于是 `stage-companions` 不生成 `studio-resources/agent/sandbox/`，但 `tauri-config` 仍声明它；`tauri-build` 校验 bundle 资源路径时直接报错。

实测 staged 目录当时只有 `aster-acp`（341 MB）和 `aster-office`（137 MB），没有 `sandbox/`。

## 3. 修复

在 `apps/studio/scripts/studio_product.py` 抽出公共判定 `sandbox_assets_expected()`，让两处共用同一个条件：

```python
def sandbox_assets_expected():
    if sys.platform != "darwin" or platform.machine() != "arm64":
        return False
    if os.environ.get("ASTER_DEFER_RUNTIME_COMPONENTS") == "1":
        return False
    workspace = Path(__file__).resolve().parents[3]
    return (workspace / "crates/oci-runtime/crates/cli/Cargo.toml").is_file()
```

- `stage-companions`：`if sandbox_assets_expected(): from sandbox_assets import stage; stage(...)`
- `tauri-config`：`if sandbox_assets_expected(): mapping[.../agent/sandbox/] = "agent/sandbox/"`

没有改 `tauri.conf.json`、没有伪造空目录、没有放宽 Tauri 校验；若日后 OCI runtime 回归，两端会同时重新启用 sandbox 资源。

## 4. 验证

| 检查 | 结果 |
| --- | --- |
| `python3 -m py_compile studio_product.py` | OK |
| `tauri-config` 输出 | 不再含 `agent/sandbox/`；其余 4 项资源完整（`resources/agent/`、`aster-acp`、`aster-office`、`document/`） |
| 声明的所有资源路径存在性 | **4/4 OK**（此前正是 `sandbox/` 那一项 MISSING） |
| `python3 -m unittest discover -s tests -p studio_product_test.py` | **14/14 通过**（含新增回归用例） |
| 新增回归用例 `test_sandbox_resource_declaration_matches_staging_predicate` | 断言 `agent/sandbox/` 是否被声明 **恒等于** `sandbox_assets_expected()`；并验证 defer 模式下既不声明 sandbox 也不声明 document |
| `git diff --check` | 通过 |
| `just build-studio` | 资源错误**已消失**，构建推进到下一阶段后停在另一个错误（见 §5） |

## 5. 剩余的是环境限制，不是本次修复的回归

在本会话的工具沙箱里重跑 `just build-studio`，日志里已无 `resource path ... doesn't exist`，但卡在更早的依赖编译：

```
screencapturekit-11.0.0/build.rs:285: Swift build failed
error: 'swift-bridge': Invalid manifest ...
sandbox-exec: sandbox_apply: Operation not permitted
```

这是 SwiftPM 在解析 manifest 时尝试创建嵌套沙箱、被当前执行环境拒绝；`screencapturekit` 的 `build.rs` 直接 `swift build -c release ...`，没有提供禁用该沙箱的开关。它是**工具沙箱的环境限制**（本会话前几轮记录过同一现象），不是产品代码问题——用户是在自己终端跑到 `cargo build --bin Studio` 阶段才遇到 `agent/sandbox` 资源错误的，说明其终端已越过 SwiftPM 这一步。

因此修复后的完整 native 构建需要在普通终端（非工具沙箱）里执行：

```
cd /Users/roylin/code/aster && just dev
```

若普通终端仍出现 `sandbox_apply` 报错，可单独排查 Xcode CommandLineTools / SwiftPM manifest sandbox，与本次资源声明修复无关。

## 6. 改动文件

- `apps/studio/scripts/studio_product.py`（+ `sandbox_assets_expected()`，两处条件改为复用）
- `apps/studio/scripts/tests/studio_product_test.py`（+ 1 条一致性回归用例）

未提交、未 push。
