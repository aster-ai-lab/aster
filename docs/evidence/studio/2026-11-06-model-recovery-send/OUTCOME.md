# 模型失败后恢复并实际发送的闭环验收

日期：2026-11-06

## 结果

**通过。** 在真实 Studio 窗口中依次完成：模型加载失败、恢复模型、点击重试、发送任务、批准写入、收到最终回复并验证磁盘文件。

本项是在 `2026-11-06-model-recovery-ux` 的失败界面验收之后继续完成的闭环验证，没有修改模型选择器或另一进程负责的组件。

## 构建

使用当前源码重新构建：

```bash
cd /Users/roylin/code/aster/apps/studio
ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=.studio/dist/model-recovery-ux \
  node node_modules/@rsbuild/core/bin/rsbuild.js build

cd /Users/roylin/code/aster
ASTER_DEFER_RUNTIME_COMPONENTS=1 just build-studio
```

两次构建均成功。窗口使用延迟可选沙箱组件的宿主和 `44362` 渲染预览。

## 实际验收步骤

1. 在隔离实例中禁用 `openai` 提供商并重启宿主，避免复用禁用前已预热的运行时。
2. 打开 `WRITE-E2E` 任务，窗口显示“当前模型不可用”，确认真实失败状态。
3. 恢复提供商配置并点击“重试”。错误面板消失，`gpt` 与输入区恢复可用。
4. 在窗口输入并发送 `WRITE-E2E`。
5. 窗口出现 `write` 权限确认后，实际点击“允许”。
6. 最终窗口显示“已应用”和 `Write completed.`。
7. 重新读取会话工作区内的 `gate-target.ts`，内容为 `hello from Studio`。
8. 最终窗口与模型请求中均未出现 `completion gate: workspace mutation`、`当前模型不可用` 或 `模型仍不可用`。

恢复后的主任务使用 2 次模型请求完成工具调用和最终回复，另有 2 次无工具的会话标题请求。没有补跑 `test -f` 或 `test ! -e`。

## 证据

- [模型失败窗口控件记录](artifacts/model-failed-window.txt)
- [模型恢复窗口控件记录](artifacts/model-recovered-window.txt)
- [实际写入成功截图](artifacts/send-completed.png)
- [实际写入成功窗口控件记录](artifacts/send-completed-window.txt)
- [请求摘要](artifacts/requests.txt)
- [原始模型请求](artifacts/model-requests.jsonl)（本次恢复运行从第 6 条开始；前 5 条属于前一项场景）

## Explicitly not claimed

- 不宣称完整发布包或可选沙箱组件通过验证；本次使用延迟组件构建。
- 不宣称真实外部模型服务可用；本次使用本地确定性 Responses API 模拟器。
- 不宣称其他模型、权限模式、模型选择器或 GUI 流程通过验证。
- 不把模型最终文本单独视为成功证据；通过结论同时依赖实际工具完成、窗口“已应用”和文件读回。
