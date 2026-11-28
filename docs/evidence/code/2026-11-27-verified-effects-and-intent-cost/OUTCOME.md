# 已有变更回执时跳过只读意图分类

先前的六轮 DeepSeek 记录里，每次 Aster 运行都发起六次简短的预分析请求。按请求顺序和本轮已提交的工具回执逐项核对，五次发生在本轮文件写入之后。每次这类请求共使用 267 token。这个入口用于只读探索达到预算后的收尾决策。[先前六轮原始厂商回执](../2026-11-27-native-batch-read-reuse/long-a/provider-usage.jsonl)

Core 复用完成绑定已经校验过的原始回执。只读分类达到预算时，只有当前运行、会话线程和能力绑定都相符，并有完整工作区变更前后值，才可关闭这个分类入口。无变化、只有内部状态文件、缺少前值、未完整观测或属于其他运行的回执不触发。预先启用的规划与显式样式仍由既有入口选择，现有工具授权、模型交付、完成门槛和记忆提取继续运行。

## 权限与运行范围回归

- `cargo test --locked -p aster-core --lib settled_workspace_change_skips_lazy_classification_for_its_run_only -- --nocapture` 在修改前复现已完成两次工具操作并成功修改文件，仍调用一次只读意图分类。
- `cargo test --locked -p aster-core --lib -- read_only_delivery workspace_change_proof receipt_restore_tests tool_history_tests --test-threads=4`，84 项通过。写入回合成功并取得宿主绑定核验，分类调用为 0。后续独立只读回合真实打开两个文件，仍发起一次分类并保留收尾指引。类型回执覆盖旧运行、错能力绑定、不完整结果、缺少前值、无变化、内部路径和不匹配的准备事实。

## 写入后的精确命令

复跑已冻结的 `verified_edit_exact_count`，两个版本各 2/2。两组都实际执行了原样命令，退出码为 0，文件字节和绑定核验通过。每次原版运行在读与编辑后额外调用一次分类，使用 262 token。新版为 0 次，且每次运行仍有两次记忆提取。整个两轮任务的 token 中位数从 34,978 降至 34,719。[命令与用量摘要](verified-edit-summary.json) · [原判分](verified-edit/trials.json)

## 六轮 DeepSeek 对照

使用同一个冻结夹具和判分，两组 Aster 都显式启用按需目录。DeepSeek Flash 关闭思考，temperature 0，上限 4,096 个输出 token。两次运行顺序分别为 `candidate release dsh` 和 `dsh release candidate`，独立 DSH 版本为 `0.2.2-rc.5`。171 次请求均返回 HTTP 200 和完整用量。

| 六轮结果 | 原版 Aster | 新版 Aster | 独立 DSH |
| --- | ---: | ---: | ---: |
| 总 token 中位数 | 185,009.5 | 178,258.5 | 265,762.5 |
| 未命中缓存输入 token 中位数 | 15,565 | 15,558.5 | 7,412 |
| 分类次数 | 6 | 1 | 0 |
| 分类用量 | 1,602 | 267 | 0 |
| 记忆提取次数 | 6 | 6 | 0 |
| 记忆提取 token 中位数 | 8,698.5 | 8,846.5 | 0 |
| 正确产物和输入保护 | 12/12 | 12/12 | 12/12 |
| 严格工具通过 | 0/2 | 0/2 | 2/2 |

两次候选各少五次分类，均直接减少 1,335 token。保留的一次发生在首次写入前。两组 Aster 的全部 24 轮都获得宿主绑定核验，且每次运行各有一次额外 `ls`。新版本组总量中位数比独立 DSH 低 32.93%，总量差异也包含模型工具选择和回复变化。第一轮新版总量略升，反向顺序下降。已按用量回执拆分分类、记忆提取和主请求。[正向判分](long-a/trials.json) · [反向判分](long-b/trials.json) · [用量与回执序列汇总](long-summary.json)

## 本机构建

- `cargo test --locked -p aster-acp --test long_horizon_real_llm --no-run` 和 `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` 通过。macOS arm64，仓库固定 Rust 1.97.1，`apps/studio` 已初始化。候选配套 ID 为 `aster-cli-pair-v1:4dc4285a874c8a93d842fa964ce6113fb41a084b:aarch64-apple-darwin`，ACP SHA-256 为 `6a910ed15967cdcbbba83f3ea937ff185021668ab0e7908268294d58daf369be`。原版 ACP SHA-256 为 `ec41951909931c8bf941c495cd09ac6e13001c7d5477d1d166006e5bf17eddb8`。
- 对候选配套运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。排除项见[原生沙箱基线](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)。
- `cargo check --locked --workspace --all-targets`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live`、定向 `rustfmt`、`git diff --check` 与评测脚本语法检查通过。全工作区 `cargo fmt --all -- --check` 仍报告同样的 38 个既有差异文件。
- `kimi-01/k3.1-pro` 以候选配套运行仓库的 `real_native_file_workflow_multiple_user_turns --ignored --exact --nocapture`。六轮产物、输入保护和宿主绑定核验均通过。28 次上游 HTTP 200 均取得用量，总计 200,713 token。[Kimi 原始判分](kimi-result.json)

## Explicitly not claimed

依赖当前运行的确凿变更回执，所以写入前达到探索预算的回合可以继续进行分类。测量未用于估计账单费用，原六轮任务的严格工具约束应和产物正确性分别核对。
