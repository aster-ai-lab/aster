# 复用已经通过的宿主读回核验

上轮六次长任务中，Aster 四次运行一共提交了 24 处文件改动。用原始工具回执和通用核验提示重建工具消息，再与每次真实厂商请求保留的消息 SHA-256 核对，24 次均有提示。随后有 19 次 `read` 再读同一个刚改动的文件。原提示只说改动已核验，模型没有获知宿主具体比较了什么。 [前一轮逐次对账](previous-postwrite-decisions.json) · [原始长任务记录](../2026-11-27-six-turn-dsh-comparison/OUTCOME.md)

Core 现在把当前通过的宿主磁盘读回标为具体的核验类别。只有本次宿主预览返回绑定到当前改动的标准磁盘核验报告，而且完成门槛确实确认 `Verified` 时，才说明宿主已将磁盘内容与记录的效果进行核对。用户要求其他检查、需要新内容或文件后来变化时，模型继续做相应工作。它仍按原样使用用户指定的命令。其他来源的验证报告沿用原提示，过期报告和新用户回合不会获得新的读回提示。原工作区访问、审核、报告提交和完成门槛按既有路径运行。共享仓库工具说明要求遵守显式工具限制并对已知路径直接执行获准的文件操作。

## 初次措辞试验

同一官方 DeepSeek Flash，关闭思考，temperature 0。原版 ACP SHA-256 `ea4e2807e9dd8d9dd409870b649fa51b0254e967eb07ca7766200c78d941b083`。初次候选 SHA-256 `b66c786592088336bdb1ab185ecad822ca00e86afcf2e5e2a926cb8ccb1264ed`。六轮试验两次都通过全部文件与宿主核验，从 27 次主请求降至 21 次。候选使用现有 `read.files` 一次批量读取确定的多个路径，两个样本仍各有一次额外 `ls`。在启用磁盘预览且用户另外要求精确命令的独立短任务中，初次候选 0/2，原版 1/2。模型确实提出了 Bash 调用，但有三次在原命令后附加了别的命令，宿主拒绝执行，首轮缺少真实输出。失败文件和原判分保留。随后明确要求独立检查遵守给定的精确命令。[初次六轮对照一](initial/long-a/trials.json) · [对照二](initial/long-b/trials.json) · [独立检查首测](initial/verify-with-preview/trials.json)

## 最终六轮对照

两次交替顺序使用同一份现有夹具和独立判分，Aster 两组都显式启用按需目录。厂商回执继续使用 4,096 的输出上限。独立 DSH 为 `0.2.2-rc.5`。最终配套构建 ID 为 `aster-cli-pair-v1:aef189bc9177a20a30f6841155cecadea55c87bd:aarch64-apple-darwin`，ACP SHA-256 为 `59d46e5709faa55c19f8227aaf9ae822d27ed2725e337f2c37fa5220f3721e99`。

| 六轮任务 | 文件与保护判分 | 严格工具判定 | 总 token 中位数 | 未命中缓存输入中位数 | 主请求中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 上一版 Aster 按需目录 | 12/12 | 0/2 | 209,768 | 18,903 | 23.5 |
| 新版 Aster 按需目录 | 12/12 | 0/2 | 170,350 | 17,726.5 | 20 |
| 独立 DSH | 12/12 | 2/2 | 252,143.5 | 7,006 | 24.5 |

新版按需目录总量比上一版低 18.79%，比独立 DSH 低 32.44%。核对 Aster 原始工具回执，两个版本在每轮写入前都取得当轮 `README.md`、`order.json` 和 `policy.json` 的实际内容，全部 24 轮均取得宿主绑定核验。在同一对照中，写后发出的文件读取从 7 次减至 1 次。两个新版按需样本各有 1 次额外 `ls`。完整目录额外复跑一次，209,538 token、20 次主请求，6/6 轮文件和核验通过，但仍有 2 次额外 `ls`。包括完整目录在内一共 216 份厂商回执，全部取得用量。[顺序一](final/long-a/trials.json) · [顺序二](final/long-b/trials.json) · [完整目录](final/adaptive/trials.json) · [每轮所读数据](current-read-receipts.json) · [汇总](final-long-summary.json)

## 独立检查和短任务

用户额外要求精确 Bash 命令的任务，最终候选在已有磁盘核验时 2/2 通过，未启用磁盘预览时同样 2/2。候选四次都执行了原样命令，获得退出码 0 和预期输出，写入字节与绑定摘要符合原判分。旧版在启用预览时 1/2，有一次添加未获准的命令。[命令与工具回执](independent-check-receipts.json) · [启用磁盘核验](final/verify-with-preview/trials.json) · [默认核验](final/verify-default/trials.json)

原两轮冻结任务 9/9。上一版 Aster 按需、新版按需和独立 DSH 总 token 中位数分别为 23,192、23,396 和 25,057。新版比 DSH 低 6.63%，比上一版高 204 token。额外独立范围覆盖 MCP、分页、搜索、Program、Batch 与两种核验姿态。所有短任务中候选 17/17，上一版 16/17，DSH 3/3。[冻结任务](final/frozen/trials.json) · [全部最终对照](final/)

## 本地验证

- `cargo test --locked -p aster-core --lib assistant_delivery_receives_current_disk_attestation_instead_of_an_extra_command` 在修改前复现提示缺失。
- `cargo test --locked -p aster-core --lib -- final_delivery verification_preview completion_attestor tool_history_tests prompts::tests::test_repository_tool_contract --test-threads=4`，最终 55 项通过。
- `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp`，成对构建成功。用最终的 `aster` 和 `aster-acp` 运行 `boundary_e2e --skip e_bash --nocapture`，9 项通过。排除的 `e_bash` 在[已有证据](../2026-11-27-host-denial-and-pending-verification/OUTCOME.md)记录。
- `cargo check --locked --workspace --all-targets`、`cargo check --locked -p aster-core --features ci-all,deep-live`、`cargo clippy --locked -p aster-core --all-targets --no-deps --features ci-all,deep-live` 和 `git diff --check` 通过。全工作区格式检查仍有 38 个已有差异文件。
- 用[原六轮运行器](../2026-11-27-six-turn-dsh-comparison/revisions.py)、[原冻结运行器](../2026-11-27-model-selected-tool-catalog/native.py)和[短场景运行器](../2026-11-27-model-selected-tool-catalog/full_catalog.py)复跑。独立检查以 `--scenario verified_edit_exact_count --native-catalog --reference-acp "$BASELINE_ACP" --acp "$ACP"` 为基础，启用磁盘对照加 `--disk-content-attestation`。各目录中的 `manifest.json` 记录一手输入、模式和试验顺序。

## Explicitly not claimed

结果覆盖所列任务和关闭思考的 DeepSeek Flash。Aster 六轮的严格工具约束仍未全部通过，未验证跨模型与其他平台的全面等价，也未估算费用。按需目录仍需显式启用。
