# 错误可见面的第一性原理收口（2026-11-11）

## 1. 先更正上一轮的判断

上一轮曾判断「`providerErrorText` 会直取 `error.data` 导致门禁文本泄漏」。**该判断不成立**：`src/shared/ai/providerError.ts` 的 `payloadText()` 对纯字符串 `data` 走 `JSON.parse` 失败分支并返回空串，只会从结构化 JSON 的白名单字段（`error.message` / `message` / `detail` 等）提取文本。本轮用可执行断言重新定位了真实漏点。

## 2. 第一性原理推导

用户发起任务后需要的是「成没成 + 结果 + 下一步」，不是运行时执行轨迹。当前架构把 ACP 事件与错误载荷投影成对话内容，再靠 `userFacingText.ts` 的正则、`redactInternalPaths`、`isInternalProcessError` 逐层遮盖。**字段白名单不等于内容白名单**：只要内部诊断文本恰好装进白名单字段，就会穿出来；每换一个承载字段就要补一次判定，这是补丁循环，不是解法。

0.2.1 曾因同样的用户截图修过一次（`studio 818b3ac5a1` / `7b20a40966`，见 `docs/evidence/studio/2026-11-09-release-0.2.1-attempt/OUTCOME.md:229`），当时只在 `ErrorBlock` 判定 `error.message`，回归断言也只构造 `message` 形态，因此被其他载体绕过。

## 3. 先测准漏点（可执行事实）

在 `ErrorBlock.test.tsx` 加入参数化契约，覆盖同一个门禁文本（`completion gate: workspace mutation … has no bound Passed verification …`）的四种载体：

| 载体 | 修复前 |
| --- | --- |
| `error.message` | 不泄漏（已有 `isInternalProcessError`） |
| `data` 纯字符串 | 不泄漏（`payloadText` 返回空） |
| `data = { message: ... }` | **泄漏** |
| `responseBody = {"error":{"message":...}}` | **泄漏** |

修复前实跑：`22 passed / 2 failed`，两处失败均可见整段英文诊断出现在卡片正文（`ErrorBlock.test.tsx:563` 断言）。

## 4. 改动（全部在 `apps/studio` 渲染层）

| 文件 | 内容 |
| --- | --- |
| `utils/userFacingText.ts` | 新增 `isInternalProcessErrorPayload(error, depth)`：判定**整个错误对象**，递归字符串 / 数组 / 对象 / JSON 字符串（深度上限 4），任一自由文本命中内部门禁特征即整体拒绝 |
| `components/chat/messages/blocks/ErrorBlock.tsx` | 正文与 hover 预览两个可见出口共用该函数；命中时统一渲染 `error.agent.rpc.internal.message`。移除原先只看 `error?.message` 的窄判定。原始 `diagnosticText` 仍完整保留给「详情弹窗」与「复制日志」 |
| `__tests__/ErrorBlock.test.tsx` | 新增 4 形态参数化契约 + 保留原有 20 项行为 |
| `utils/__tests__/userFacingText.test.ts` | 新增 2 条：嵌套 / JSON 载体可识别、普通 provider 错误不误判 |

**不改动**：Rust core / ACP 协议、i18n 文案、provider 原文案的既有展示、hover 对普通错误的原始诊断预览、AI summary、工具活动层。既有 20 项测试全部保持通过，未放宽任何断言。

`git diff --check` EXIT=0。

## 5. 实际验证（命令与计数）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| 改动相关单测 | `node_modules/.bin/vitest run --project renderer src/renderer/components/chat/messages/blocks/__tests__/ErrorBlock.test.tsx src/renderer/utils/__tests__/userFacingText.test.ts` | **2 files / 35 tests passed**（ErrorBlock 24、userFacingText 11） |
| 性能布局契约（等价入口） | `node_modules/.bin/vitest run --project renderer <perf-layout-contracts.json 的 9 个路径>` | **9 files / 185 tests passed** |
| renderer 类型检查 | `node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false` | EXIT=2，**309 个预存在错误**；本次 4 个改动文件 **0 错误**。首次运行曾报 `ErrorBlock.test.tsx(559,41) TS2783 'message' is specified more than once`（本次引入），已通过移除重复键修复并复核 |
| 空白 | `git diff --check` | EXIT=0 |

官方 `just studio-contracts` 未运行：本机 Node 22.22.2 低于 `engines >=24.12`，pnpm 报 `ERR_PNPM_UNSUPPORTED_ENGINE`（与上一轮相同的环境限制），故用等价 vitest 入口覆盖同一批 9 个文件。

四元组：通过 35 + 185；失败 0（最终轮）；跳过 0；未执行：官方契约入口、真窗口复现。

## 6. 剩余边界（这才是「深度」的下一层）

本轮把判定面从「单字段」提升为「整个错误对象 + 所有可见出口共用收口」，但**仍依赖文本特征识别内部诊断**，没有彻底消除「运行时自由文本进入可见面」这条通路。彻底解需要：

1. **Rust 侧给机器错误一个稳定 code**：`crates/code/core/src/agent/loop_runtime.rs:576-581` 生成的 completion-gate 错误应携带结构化 `code`（如 `completion_gate_unverified`）而非把 digest / 字段名写进 message；ACP 透传该 code；渲染层按 code 映射 i18n，永不渲染其文本。这样才真正不需要任何正则。
2. **成果叙事与执行轨迹分通道**：截图里除错误卡片外，「询问用户 / 列出文件 / 4 次工具调用 / 已删除并确认不存在 … / 目录 UUID」都是执行轨迹。理想形态是默认只展示成果摘要，轨迹折叠进显式详情。这属于产品交互决策，未擅自改动。
3. **其他仍可能泄漏的出口**：AI summary（标题 `aiSummary || localizedErrorMessage`）、模型 prose（`MainTextBlock` 的 `stripInternalProcessText` 只删整行以 `completion gate:` 开头的行）、工具活动里的绝对路径（依赖每个渲染点是否调用 `redactInternalPaths`）。

## 7. Explicitly not claimed

- **未声称解决了「所有内部细节暴露」**：只收口了错误卡片正文与 hover 两个出口；工具活动、AI summary、模型 prose 未动。
- **未声称脱离文本判定**：`isInternalProcessErrorPayload` 仍基于文本特征；彻底解需要 Rust 侧结构化 code，本轮未改 Rust / ACP。
- **未声称官方契约门禁通过**：`just studio-contracts` 因环境未跑，通过的是等价入口的 185/185。
- **未声称 renderer 类型检查通过**：309 个预存在错误仍在。
- **未声称真窗口行为已验证**：本轮仅代码与单测验证，未重新构建并驱动窗口复现。
- **未声称截图对应的版本就是当前代码**：截图 digest `f9066392cac903…` 与 `userFacingText.test.ts:22` 样本前缀一致，无法据此判断是新结果还是旧图；本轮结论基于当前 checkout 的可执行测试。
- **未声称已交付**：`apps/studio` 子模块为脏树（本轮 4 文件 + 上一轮推理档位修复 3 文件，共 7 文件 / +164 −13），未 commit、未 push、未 bump 父 gitlink；推送需另行授权。
