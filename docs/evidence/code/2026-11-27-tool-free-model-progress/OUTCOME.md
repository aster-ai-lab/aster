# 无工具请求与模型输出恢复

捕获的 DeepSeek 请求包含零个工具定义，但模型仍能在正文里输出 DSML 调用。此前 Core 在响应解析阶段把这些标记恢复成工具调用，事实循环因此新增拒绝回执，再发起一次主模型请求。基线探针记录了该次序。保留请求和响应的结构计数，原始正文只保存在临时诊断目录。[基线探针](baseline-probe.json)

Core 根据实际交给模型的工具定义添加无工具说明。此时原生工具调用或正文中未被引作代码的调用标记不会进入工具决策。出现协议不合时，沿同一宿主治理路径，给模型一次明确的文本纠正机会。纠正请求保留原上下文和授权约束，并重新检查取消。第二次仍返回不可执行调用时退出，保留错误状态。行内与围栏代码的字面标记通过工作区已锁定的 Markdown 解析器识别并保留。只开放了已有依赖对 Core 的使用，锁文件记录增加一项。

提供工具的请求沿用原生调用和泄露协议恢复流程。宿主授权、人工确认、工具事实和核验判定仍在原路径处理。首次模型调用和实际发起的纠正调用分别计量，不生成虚构工具回执。请求中没有工具时，常驻上下文多一条简短的可用性说明。

## DeepSeek Flash 实测

macOS 27.2.1 arm64，Rust 1.97.1，DeepSeek Chat Completions。关闭思考，temperature=0，输出上限 1024。无工具对照使用上一轮的 SDK 英文库存对账和中文翻译审计，同一测试、同一合并规划选项和同一工具隔离配置。A/B 分别运行前后版本的 SDK 测试二进制，每个场景交替执行三轮。统计各列独立中位数。[对照驱动](compare-tool-free.py)

| 场景 | 指标 | 优化前 | 优化后 | 变化 |
| --- | --- | ---: | ---: | ---: |
| 无工具英文库存对账 | 主模型请求 | 3 | 1 | −2 次 |
| 无工具英文库存对账 | 主模型总 token | 5,155 | 2,149 | −58.31% |
| 无工具英文库存对账 | 全程总 token | 5,927 | 2,873 | −51.53% |
| 无工具中文翻译审计 | 主模型请求 | 5 | 1 | −4 次 |
| 无工具中文翻译审计 | 主模型总 token | 8,596 | 2,065 | −75.98% |
| 无工具中文翻译审计 | 全程总 token | 9,388 | 2,851 | −69.63% |
| 原冻结任务 | 全程总 token | 33,346 | 33,336 | −0.03% |
| 重复观测 | 全程总 token | 38,034 | 38,025 | −0.02% |

无工具对照 12/12 通过既有规划和回合判分，43 份厂商用量齐全。候选 6 次任务均只发起 1 次主请求。候选有 1 次中文最终正文到达厂商输出上限，宿主保留截断状态。此前同一用例已记录过拒绝循环失败，本次对照的旧版与新版都达到既有通过条件。[无工具判分](tool-free/trials.json) · [厂商用量](tool-free/provider-usage.jsonl)

原冻结任务 9/9 通过，48 份用量齐全。同一任务的 DSH 0.2.2-rc.5 为 24,919 token。新版 Aster 和 DSH 的缓存命中输入分别是 31,872 和 23,680 token，未命中输入分别是 1,184 和 831 token。独立重复观测任务 6/6 通过，36 份用量齐全。两组工具任务用量变化很小，主请求次数中位数为 4。所有对照合计 27 次任务和 127 份用量。[冻结参数](frozen/manifest.json) · [冻结判分](frozen/trials.json) · [冻结用量](frozen/provider-usage.jsonl) · [重复观测参数](repeated/manifest.json) · [重复观测判分](repeated/trials.json) · [重复观测用量](repeated/provider-usage.jsonl)

复制基线的首个英文和中文主请求进行诊断。单独追加无工具说明后，两份请求都返回正常正文，基线都返回了调用标记。这组诊断用于定位原因，正式百分比来自上表的实际会话。[诊断用量](ablation.json) · [各组中位数](medians.json)

- SDK 旧版测试二进制 SHA-256 `a18bbb5fb334603f18e599157b7c7adfd5fa5c2636b636eb34353bc3e9a12f78`，新版 `a395a576d3df8e58739a45ace71aaef685dafe9f7b7f0ccc143d0861e2b59ef3`。
- ACP 旧版 SHA-256 `911ad5780ef178f37ea5007fd3c6a72737a9cdd2a098111085e5d9f39f05c7df`，实测新版 `03d15213fbf0f9f86902926964d8676d71bdc01fa43a12a886d6d59dd9379fc5`。

## 本地验证

| 命令或场景 | 结果 |
| --- | --- |
| `cargo build --locked -p aster-core -p aster-acp --lib --bin aster-acp` | 通过。 |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | 通过，两件产物构建 ID 均为 `aster-cli-pair-v1:5023bb258c7006b2dca5afbdc0aeff6549d06b82:aarch64-apple-darwin`。 |
| `cargo check --locked --workspace --all-targets` | 通过，保留已有警告。 |
| `cargo check --locked -p aster-core --features ci-all,deep-live` | 通过。 |
| `cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` | 通过。按变更范围复核诊断，无需修改本轮实现。日志仍包含相邻测试和其他模块的已有警告。 |
| `cargo test --locked -p aster-core --lib -- fact_control:: agent_api::tests::read_only_delivery agent_api::tests::stream llm::text_tool_calls --test-threads=4` | 312 项通过。覆盖已提供工具的恢复、无工具原生与文本调用、私有推理、纠正上限、字面标记和事实回执。初次发现一个历史回放夹具缺少显式授权，已补齐夹具并断言第一轮实际提供 read，保留原来的回放断言。 |
| `cargo test --locked -p aster-acp --lib -- engineering prompt_boundary retrieval_seam count_limit session_setup stop_interrupts_unanswered_permissions_questions_and_forms --test-threads=4` | 40 项通过，3 项按既有标记跳过。 |
| `cargo test --locked -p aster-core --test test_control_loop_deepseek_e2e --no-run` | 构建通过，所得二进制用于真实 SDK 对照。直接运行真实核验写入和目标重开用例，2 项通过。 |
| `real_agent_recovers_from_an_outside_workspace_refusal_in_the_same_turn` | 真实 DeepSeek + 候选 ACP，1 项通过。先越界拒绝，再同轮完成已授权的读取。 |
| `forms_catalog::kimi_catalog_native_viewport_and_typed_values_match_source` | 使用最终配对二进制，真实 Kimi 原生 PTY 测试 1 项通过。涵盖 13 种组件、8 个带类型的字段和输入文件保真。[原生断言](native-kimi.txt) |
| `/opt/homebrew/bin/python3.12 tests/workspace_dependencies.py` | 7 项通过。系统 Python 缺少 tomllib，改用已安装的 Python 3.12 后通过。 |
| `cargo fmt --all -- --check` 与 `git diff --check` | 通过。 |

## Explicitly not claimed

- 降幅用于上述工具为空且基线存在协议泄露的会话。零工具但基线一次作答的普通请求会新增简短宿主说明。
- 本轮按现有判分比较，原生模型正文的截断样本已经列出。测试没有验证所有任务或所有模型的可靠性。
- 账单与未命中缓存用量是单独指标。Windows/Linux 实机和 Studio GUI 未纳入此次运行。
