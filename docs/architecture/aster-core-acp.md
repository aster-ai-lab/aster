# Aster Core 与 ACP 技术 DeepWiki

> 文档范围：`crates/code/core` 与 `crates/code/acp` 的当前实现，重点是工作区搜索、检索持久化、取消、失败语义和发布验证。
>
> 当前版本：Core／ACP `9.1.8`，CLI `0.1.8`。文档更新于 2026-11-05。
>
> 本文描述当前工作树，不代表全部本地改动已经稳定，也不代表已经发布新版本。

## 1. 系统概览

Aster 将终端交互、协议适配和执行内核分成三层：

```text
用户 / Host
   │
   │ ACP initialize、session/new、prompt、cancel
   ▼
aster-acp
   │
   │ AgentSession、SessionOptions、AgentEvent
   ▼
aster-core
   │
   ├── 模型与工具编排
   ├── 权限、完成检查和事实记录
   ├── WorkspaceServices
   │      ├── LocalWorkspaceBackend
   │      └── ManifestWorkspaceBackend
   └── WorkspacePersistentIndex（可选 aster-vec FTS）
```

核心约束如下：

1. **CLI 与 ACP 必须成对构建和更新**，共享 BUILD_ID。
2. **搜索必须在有界资源内结束或返回部分结果**，不能因目录遍历阻塞整个会话。
3. **取消不能依赖停止 `spawn_blocking` 的 JoinHandle**，必须由工作线程协作检查取消标志。
4. **部分扫描结果必须显式标记为不完整**，不能伪装成全量“没有匹配”。
5. **增量索引必须保留稳定的文档主键和结果序号映射**，删除和追加不能复用仍存活的主键。
6. **错误分类必须来自类型，不依赖渲染后的错误文本**。
7. **构建、测试和发布是不同门禁**。真实任务通过不等于完整发布验收；本地候选包通过不等于已经发布。

主要源码入口：

| 范围 | 入口 |
| --- | --- |
| ACP 会话创建 | [`agent/session_open.rs`](../../crates/code/acp/src/agent/session_open.rs) |
| Core 工作区服务 | [`workspace/services.rs`](../../crates/code/core/src/workspace/services.rs) |
| 本地文件与搜索 | [`workspace/local.rs`](../../crates/code/core/src/workspace/local.rs) |
| 清单检索 | [`workspace/manifest.rs`](../../crates/code/core/src/workspace/manifest.rs) |
| aster-vec 适配 | [`retrieval/aster_vec.rs`](../../crates/code/core/src/workspace/retrieval/aster_vec.rs) |
| 持久化代际 | [`retrieval/persistent.rs`](../../crates/code/core/src/workspace/retrieval/persistent.rs) |
| 目录协调与失败类型 | [`retrieval/reconcile.rs`](../../crates/code/core/src/workspace/retrieval/reconcile.rs) |
| 后台协调器 | [`retrieval/runtime.rs`](../../crates/code/core/src/workspace/retrieval/runtime.rs) |

## 2. ACP 会话装配

### 2.1 初始化顺序

ACP 新建会话时，主要顺序为：

```text
ensure_core_agent
   │
   ▼
session_options_for
   │
   ├── 读取 workspace_retrieval 配置
   ├── 准备可选的本地 embedding provider
   ├── 合并宿主提示词和工具配置
   └── 保留工作区 operation_timeout
   │
   ▼
Agent::session_async / resume_session_async
   │
   ▼
prepare_backend_services
   │
   ├── 创建 ManifestWorkspaceBackend::new(write_root)
   └── WorkspaceServices::local_with_retrieval_backend
   │
   ▼
start_workspace_indexing
```

相关实现：

- [`AsterCodeAgent::ensure_core_agent`](../../crates/code/acp/src/agent/session_open.rs)
- [`AsterCodeAgent::session_options_for`](../../crates/code/acp/src/agent/session_open.rs)
- [`AsterCodeAgent::open_core_session`](../../crates/code/acp/src/agent/session_open.rs)
- [`prepare_backend_services`](../../crates/code/core/src/agent_api/capabilities.rs)

### 2.2 普通与无人值守会话

| 会话类型 | 工作区路径 | 检索与状态 | 本文验证 |
| --- | --- | --- | --- |
| 普通 ACP | 默认本地工作区装配 | 沿用交互式权限和会话状态 | 真实 ACP initialize/new/prompt，搜索预算测试通过 |
| `--unattended` | 默认清单／检索工作区装配 | 私有状态根、无人值守权限、默认检索能力 | 真实 ACP initialize/new/prompt，搜索预算测试通过 |

两条路径都进入 Core 的工作区服务，没有测试专用的生产预算开关。无人值守会话不会因为后台运行而取消搜索限制。

### 2.3 配置继承

`WorkspaceServices::local_with_retrieval_backend` 保留 `operation_timeout`。
本地和清单工作区构造均使用 **30 秒操作超时**。这是一项后端操作边界，不是整个模型任务的墙钟 SLA。

## 3. 搜索调用链

模型通常调用统一的 `search` 工具：

```text
search(mode="grep", query, path, include, output_mode, limit)
   │
   ▼
SearchTool::adapted_args
   │
   ▼
GrepTool::execute
   │
   ▼
WorkspaceServices::run_with_timeout("grep", ...)
   │
   ├── Manifest 尚未就绪 → LocalWorkspaceBackend::grep_with_sources
   └── Manifest 已就绪   → ManifestWorkspaceBackend::grep_with_sources
```

参数适配保留以下语义：

- `query` 转为后端正则表达式。
- `path` 转为工作区相对搜索根。
- `include` 转为 glob 过滤。
- `output_mode="content"` 返回匹配内容；元数据模式可以进行只计数搜索。
- `limit` 和 `cursor` 使用现有分页合同，不改变工具权限。

### 3.1 本地搜索

`LocalWorkspaceBackend::grep_with_sources` 在 `spawn_blocking` 中执行目录遍历和文件读取。它避免在 ACP 当前线程的 `LocalSet` 内执行同步 I/O，从而让会话事件、取消和其他任务继续被轮询。

本地扫描具备：

- 正则和 glob 入口校验。
- 工作区访问边界和敏感路径拒绝。
- 单文件文本大小限制。
- 时间／文件数预算。
- 协作取消。
- 部分结果和 `truncated` 标记。

### 3.2 清单搜索

清单就绪后，搜索优先复用不可变快照中的文件清单，避免每次重新遍历目录。

执行顺序为：

1. 读取当前 `ManifestSearchSnapshot`。
2. 计算近期文件优先级和候选路径。
3. 在阻塞工作线程中执行候选索引选择和精确正则扫描。
4. 按既有优先级返回结果和来源路径。
5. 达到预算时保留已得到的部分结果并设置 `truncated = true`。

清单尚未形成首个快照时，直接回退到本地搜索，不等待索引完成才能使用工具。

### 3.3 Trigram 候选索引

自动 trigram 索引只用于候选剪枝，**不是匹配权威**。最终命中仍由工作区正则扫描决定。

以下查询不构建自动索引：

- 包含正则元字符、不能安全转换为纯文本针的查询。
- 纯文本针长度小于 3 字节的查询。

这消除了原事故中“复杂正则本不能使用 trigram，却先同步读取整个工作区构建索引”的浪费。自定义候选提供器即使很慢，其 `select_paths` 也在阻塞工作线程执行，不再卡住 ACP executor。

## 4. 预算、取消与输出语义

### 4.1 预算表

| 机制 | 当前值 | 作用范围 | 到达边界后的行为 |
| --- | --- | --- | --- |
| 扫描时间预算 | 30 秒 | 本地和清单后端扫描 | 保留部分结果，`truncated = true` |
| 扫描文件预算 | 20,000 个文件 | 本地和清单后端扫描 | 保留部分结果，`truncated = true` |
| 工作区操作超时 | 30 秒 | `WorkspaceServices::run_with_timeout` | 返回结构化超时错误 |
| 单文件文本上限 | 16 MiB | 文件读取、范围读取、搜索读取 | 拒绝或记录跳过，不无界分配 |
| 索引失败样本 | 最多 3 条 | 部分目录协调日志 | 有界诊断，同时输出类型计数 |

这些预算彼此独立。模型推理、ACP 协议等待、结果渲染、索引建设和命令调度可能额外耗时，因此 **30 秒不是端到端完成承诺**。

### 4.2 协作取消

本地 Git／grep 工作线程使用 `CancelGitWorkerOnDrop`：

```text
调用方 Future 被 drop
   │
   ▼
CancelGitWorkerOnDrop::drop
   │
   ▼
AtomicBool = true
   │
   ▼
工作线程在处理下一个文件前检查并退出
```

`spawn_blocking` 的 JoinHandle 被 drop 时，操作系统线程不会自动停止，所以取消标志是必需的。工作线程完成或取消保护被解除后，正常返回结果。

**当前边界**：候选索引构建已经移出 executor，但它本身不是可硬中断的 30 秒操作。原事故中的复杂正则已不触发该构建；不能据此宣称所有索引构建均可在 30 秒内取消。

### 4.3 截断与错误输出

截断结果与错误结果不同：

- 部分命中：返回已有匹配，并显示输出截断说明。
- 没有完成全量扫描：元数据保留 `truncated = true`，调用方不得视为完整否定。
- 后端超时：返回 `Grep search failed: ...` 的结构化错误。
- 文件过大：单独列出未搜索文件，避免把“未读取”解释为“没有匹配”。

## 5. 检索持久化与代际发布

### 5.1 持久化结构

持久化索引按 generation 组织：

```text
index/
  CURRENT
  generation-<catalog-revision>-<epoch>/
    manifest.json
    collection/
```

`epoch` 防止进程重启后相同 revision 覆盖旧代际。新代际先在 staging 目录构建，成功后重命名并更新 `CURRENT`。跨进程发布和清理由文件锁协调；查询使用共享读锁，写入和垃圾回收使用独占锁。

### 5.2 Manifest v3

v3 manifest 为每个 chunk 保存：

- chunk ID、路径、语言、行号和字节范围。
- 源文件内容摘要与 chunk 文本摘要。
- 源 revision。
- chunk 原文。
- 可选的引擎主键 `pk`。

`pk` 对 v2 manifest 的缺省值保持兼容。没有可索引 token 的 chunk 可以没有引擎文档。

重开时会验证文本摘要、chunk 身份和重复 ID。来自其他进程 epoch 的代际不能只凭较大的 revision 数字否决新 epoch 的内容更新。

### 5.3 查询序号映射

引擎主键采用 `d<N>` 形式，但它在增量代际中可以是**稀疏**的。`native_ordinals` 将引擎主键映射到当前 chunk 顺序中的位置，因此：

- 保留下来的 chunk 可移动到新顺序而不改变引擎主键。
- 追加 chunk 获得未占用的新主键。
- 重开后使用 manifest 中的 `pk` 恢复映射，而不是假定主键等于位置。

## 6. 增量更新与主键分配

### 6.1 增量流程

`plan_generation_delta` 比较前后两个 chunk 集合：

- 两边 ID 相同的 chunk 保留原主键。
- 只存在于旧集合的主键进入 `removed_pks`。
- 只存在于新集合的 chunk 进入 `appended`。
- 变更量超过阈值时改为全量构建，避免增量复制成本超过收益。

追加主键从 `previous_chunks.len()` 开始尝试，但必须跳过 `previous_pks` 中仍然存在的所有键。

### 6.2 原问题与修复

旧逻辑直接把历史 chunk 数当作下一个主键：

```text
初始：d0, d1, d2
编辑后：d0, d2, d3   （d1 被删除，d3 为追加）
再次追加：错误地生成 d3，与现有 d3 冲突
```

旧代码随后通过全量重建掩盖了功能错误，但日志出现：

```text
aster-vec delta produced a duplicate appended primary key
```

修复后：

```text
初始：d0, d1, d2
编辑后：d0, d2, d3
再次追加：d4，而不是 d3
```

### 6.3 写入端校验

`AsterVecLexicalIndex::apply_delta_at_path` 不再独立生成另一套主键，而是消费增量计划分配的 `appended_pks`。修改集合之前会检查：

- 主键数量与追加文档数一致。
- 主键非空且不含 NUL。
- 保留与追加主键没有重复。
- 每个准备写入的文档都有对应主键。

如果增量构建失败，仍保留既有全量构建回退，并记录警告，不把失败静默转换为成功。

## 7. 目录协调与失败分类

目录协调器读取 eligible manifest 文件，构建 chunk，并将结果发布为新的不可变目录快照。失败文件会形成 `CatalogReconcileFailure`，包含：

| 字段 | 含义 |
| --- | --- |
| `path` | 工作区相对文件路径 |
| `kind` | 结构化失败类型 |
| `message` | 面向诊断的错误说明 |

当前类型：

| 类型 | 来源 |
| --- | --- |
| `TooManyChunks` | 单文件 chunk 数超过当前 128 上限 |
| `BudgetExceeded(resource)` | 文件数、chunk 数、文本字节或索引字节估算超限 |
| `ReadFailed` | 文件读取或 chunk 构建任务失败 |
| `Other` | 其他目录协调错误 |

后台日志同时输出：

- `failed_files`：失败文件数。
- `indexed_files`：已入目录文件数。
- `failure_samples`：最多三条实际样本。
- `failure_kinds`：按类型聚合计数。

分类在错误转换为字符串之前完成，不用正则或关键词猜测错误含义。

### 7.1 检索准入

敏感路径、二进制、生成文件、超过 512 KiB 的文件以及 `docs/evidence` 测试产物不会进入检索目录。普通 grep 和文件读取仍可使用这些路径。

这项规则只限制检索目录，不是全仓文件访问策略，也不表示所有大型仓库都能在默认索引预算内达到 Ready。

## 8. 错误与降级语义

| 场景 | 预期行为 |
| --- | --- |
| Manifest 未就绪 | 回退到本地有界搜索 |
| 候选索引不支持该正则 | 跳过自动索引，继续精确扫描 |
| 达到扫描预算 | 返回部分结果并标记截断 |
| 调用方取消 | 设置取消标志，工作线程协作退出 |
| 增量构建失败 | 记录原因并回退到全量构建 |
| 持久化 manifest 损坏 | 拒绝该代际并触发重建路径 |
| 目录部分失败 | 发布已验证子集，同时保留失败类型和数量 |
| 语义索引降级 | 可以使用部分索引，但不能声称全量 Ready |
| 缺少沙箱组件 | 安装或执行失败，不降级为不完整发布包 |

## 9. 性能观测

以下是当前工作树中的观测值，不是跨机器性能承诺：

| 场景 | 结果 |
| --- | --- |
| 受控基准：20,005 个非匹配文件加 1 个匹配文件 | 热运行中位耗时比约 1.55×，仅适用于该夹具 |
| ACP 预算集成：20,002 个 Markdown 文件 | 普通会话 868 ms；无人值守 1,615 ms，均含 mock 模型编排 |
| 原故障真实 CLI→ACP 搜索 | 63.973 秒，退出码 0 |
| 真实仓库分析任务 | 98.502 秒，退出码 0 |
| macOS 候选包 | 13 个必需成员，共享 BUILD_ID 和 SHA256SUMS |

原事故修复前曾连续出现 120 秒超时；较早的修复验证在 32.221 秒内完成。不同模型、索引就绪状态、机器负载和会话状态不能直接作为严格前后性能对比。

## 10. 验证矩阵

### 10.1 Core 与 ACP 收敛

| 验证 | 当前结果 |
| --- | --- |
| Core 本地工作区测试 | 43 通过 |
| Core 检索测试 | 188 通过 |
| ACP 无人值守端到端 | 33 通过 |
| ACP stdio 一致性 | 14 通过 |
| 合计 | **278 通过，0 失败** |
| Core 全目标 Clippy | 通过，保留警告 |
| ACP 全目标 Clippy | 通过，保留警告 |
| 工作区依赖检查 | 7 项通过 |
| 局部 rustfmt 与 `git diff --check` | 通过 |

### 10.2 发布归档

- CLI／ACP 的 BUILD_ID 相同。
- `scripts/package-aster-cli.sh macos` 生成 13 成员候选包。
- 沙箱文件通过 `aster_sandbox.py verify`。
- 归档 SHA-256：`69c5712f891f75e5a26444aceed46bd44aef533ec8a6d9fed41d7df92f9da747`。
- 隔离安装后，普通沙箱模式返回 `Linux` 和精确随机 nonce。
- 隔离安装后，`--mode yolo` 返回 `Darwin` 和精确随机 nonce。

完整证据：

- [Core／ACP 发布收敛](../evidence/code/2026-11-05-core-acp-release-convergence/OUTCOME.md)
- [搜索预算与端到端验证](../evidence/code/2026-11-05-grep-e2e/OUTCOME.md)
- [aster-vec 增量主键与目录准入](../evidence/code/2026-11-05-aster-vec-delta/OUTCOME.md)

## 11. 构建与复现

```bash
# Core 工作区测试
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-core --lib workspace::local::tests -- --test-threads=1

# Core 检索测试
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-core --lib workspace::retrieval::

# ACP 无人值守回归
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test unattended_e2e -- --test-threads=1

# ACP 协议一致性
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo test --locked -p aster-acp --test acp_conformance_stdio -- --test-threads=1

# 静态检查
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo clippy --locked -p aster-core --all-targets --no-deps
CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=8 \
  cargo clippy --locked -p aster-acp --all-targets --no-deps

# 配对构建与沙箱准备
RUSTC_WRAPPER= CARGO_TARGET_DIR=target/grep-budget-tests CARGO_BUILD_JOBS=2 just build

# 候选包与隔离验收
scripts/package-aster-cli.sh macos <bin-dir> <archive>
python3 tests/cli_sandbox_smoke.py <archive> --output-dir <evidence>
python3 tests/cli_sandbox_smoke.py <archive> --mode yolo --output-dir <evidence-yolo>
```

全量 Core lib 测试如果出现测试栈不足，可使用 `RUST_MIN_STACK=8388608`。这只能处理测试栈配置，不能用于掩盖真实递归问题。

## 12. 当前限制与未决事项

1. **不宣称所有 Core／ACP／TUI 本地改动稳定**。本次发布候选只覆盖本文列出的搜索、索引和协议范围。
2. Core 全量 lib 测试曾接近完成但达到工具时限；单个栈溢出用例在 8 MiB 测试栈下通过，不能把未完成的套件写成全绿。
3. Pager 全量测试有 34 项失败，TUI 的完整 AUTO／MANUAL 长程旅程仍需独立结果，不属于本文的 Core／ACP 通过范围。
4. 默认索引预算不能保证大型仓库全量 Ready；部分索引可用不等于完整覆盖。
5. 候选索引构建不再是 executor 阻塞源，但还不是严格的 30 秒可取消操作。
6. 当前验证平台是 macOS ARM64。Linux 仅指沙箱客体系统，不代表 Linux 主机发布验收；Windows 和 Linux 主机未验证。
7. 全仓 `cargo fmt --all -- --check` 仍可能受到未相关的 Studio 既有差异影响，不能用局部通过替代全仓结论。
8. 当前归档是本地候选包，未提交 Git、未修改版本号、未打标签、未上传。

## 13. 后续修改指南

修改搜索或索引行为时，至少保留以下验收：

1. 本地与清单两条搜索路径都验证预算、取消和截断。
2. 用复杂正则证明不会进行无用的全量候选索引构建。
3. 用编辑、删除、重命名和重开证明稀疏主键不冲突。
4. 验证失败分类来自类型，且日志样本数量有界。
5. 验证普通 ACP 与无人值守 ACP 默认装配，不只测 Core 内部接口。
6. 使用真实 CLI→ACP 任务检查模型与协议编排，不只依赖单元测试。
7. 发布前从最终归档隔离安装，验证真实命令输出和随机输入，不只看程序能否启动。
