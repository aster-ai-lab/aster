# 删除成功后的验收绑定

用户现场的四次工具调用全部成功，删除回执将四个目标文件记录为 `missing`，随后不存在检查也成功，但没有产生绑定当前变更摘要的 `Passed` 报告。完成门因此返回未完成。已只读核对文件确实不存在，`.aster/`、`meta/` 保留；未修改原会话或重复执行删除。

现有 Host 验证报告增加纯删除的检查路径：成功的内置 bash 调用包含字面量 `test ! -e PATH`，覆盖全部删除路径，且宿主按原有类型化文件身份再次确认所有变更仍为 Missing，才生成绑定当前 effect digest 的必需检查。仍复用现有 shell 分段、字面路径解析、变更账本、元数据持久化及完成门。生产实现没有任务文件名、模型或验收答案分支。

该分支仅在没有已声明项目验收命令时生成报告。部分覆盖、检查后文件重建、混合写入、未完成观察及后台子任务不会据此取得通过。已有报告及项目验收命令继续沿用原链路。

## 验证

在用户已批准的同一个隔离 worktree 中，以 `4892ab29e` 的完整 main 源码加本修复运行。固定 Rust 1.97.1，macOS arm64；没有复制配置凭据。源码哈希、完整命令和结果见 `qualification.json`。

| 检查 | 结果 | 进程墙钟 |
| --- | --- | ---: |
| 实际 shell、宿主报告、元数据重放、完成门及拒绝场景 | 1 通过 / 1 失败 → 2 全过；失败数 −100%，通过率 +50 个百分点 | 174.383039→118.194399 秒；−32.22% |
| Core default 关联回归 | 227 通过，0 失败 | 10.753118 秒 |
| Core CLI JSON 关联回归 | 227 通过，0 失败 | 106.758977 秒 |
| Core all-target Clippy | exit 0 | 74.453620 秒 |
| CLI 与 ACP 配对构建 | exit 0 | 139.423778 秒 |
| 本次三个 Rust 文件格式 | exit 0 | 0.057785 秒 |
| 根工作区格式 | 原有 783 个文件失败，与本次文件交集 0 | 21.702111 秒 |

两个 Core 配置复用同一组选定用例，不能相加为 454 个独立场景。新回归使用独立的带空格及非 ASCII 文件名，实际执行 shell 检查；拒绝场景覆盖部分路径、检查后重建、混合写入以及已声明项目验收命令。

命令选择为 `cargo test --locked --offline -p aster-core --lib` 的 `verification::`、`harness_loop::tests::`、`agent::tool_completion_runtime::`、`fact_control::receipts::`，并分别使用默认配置和 `serde_json/preserve_order`。Clippy 使用 `cargo clippy --locked --offline -p aster-core --all-targets --no-deps --features serde_json/preserve_order`；配对构建使用 `cargo build --locked --offline -p aster -p aster-acp --bins`。完整本地 target-dir/profile 参数保留在 JSON。

有效 red 与 green 的源码基线及编译/原生加载条件不同，进程墙钟变化只作描述；其他检查没有相同历史基线，变化百分比为 N/A。所有阶段均保留计时日志。最初测试导入路径产生的 E0432 已在有效 red 之前修正，原日志保留，不计为行为回归证据。

## Explicitly not claimed

- 0 次真实模型请求；没有真实任务速度或 tokens 节省结论，两项百分比均为 N/A。
- 未重装当前 Studio/TUI，未将原会话的失败记录改为成功；运行版本更新后才能使用此修复。
- 未执行交互式 TUI 验收、GLM 原始长程 cohort 或 Windows/Linux 原生运行测试。
- 不宣称整个远程 CI 或根工作区格式通过。格式失败路径是隔离 checkout 中未修改的已提交文件。
- 其他未提交的摘要预算和运行机制不在本次提交内；本修复已在不含这些改动的 main 上独立验证。
