# 任务对话详情页消息流滚动性能优化

日期：2026-11-06

## 结果

**完成。** 在保留现有虚拟化、历史缓存、阅读锚点和底部跟随策略的前提下，减少两类无效工作：

1. 滚动位置未变化时，不再重复计算索引或写入相同的滚动锚点。
2. 消息、尺寸和回调均未变化时，虚拟列表不再因父级锚点状态变化而重渲染。

没有重写滚动运行时，没有改变 overscan、消息挂载、长距离跳转、prepend、文本选择保留或底部跟随行为。

## 修改

- `MessageList.tsx`
  - 消息组键、消息组渲染回调和滚动容器样式使用稳定引用。
- `MessageVirtualList.tsx`
  - 使用 `memo` 跳过 props 完全不变的更新，内部滚动状态仍正常更新。
- `useScrollPositionMemory.ts`
  - 底部跟随期间仅在需要清除旧锚点时写入 `null`。
  - 阅读位置和滚动偏移未变化时，提前跳过索引计算和缓存写入。
- 两个对应测试文件
  - 覆盖缓存去重，以及 500 个消息组下不变 props 的更新抑制。
- `providers_codex_probe.rs`
  - 仅为构建补充 `Ok::<TextEffort, String>` 类型标注，不改变行为。

## 自动验证

- 位置记忆、虚拟列表和完整消息列表：3 个文件、77 项测试通过。
- 完整滚动运行时：83 项测试通过。
- 总计：160 项测试通过。
- 500 个消息组：
  - 初次渲染执行 500 次消息组渲染。
  - 相同 props 再次更新仍保持 500 次，没有新增 500 次无效渲染。
  - 真实渲染回调变化则正确执行新的 500 次渲染。
- 位置记忆测试确认：重复 `null`、相同锚点和相同滚动偏移不会重复写缓存或重复读取索引。
- 相关 Oxlint / ESLint：无错误或警告。
- 渲染器构建成功。
- `ASTER_DEFER_RUNTIME_COMPONENTS=1 just build-studio`：当前源码的 Studio 构建成功。

全量 TypeScript 检查存在任务外既有错误，本次修改文件没有新增类型错误。

## 真实窗口滚动验收

使用当前源码构建的隔离 Studio 和本地确定性模拟模型，创建一条含 80 个滚动段落的长消息。

实际结果：

1. 初始底部显示段落 71–80。
2. 物理滚轮向上后显示段落 48–58，消息内容连续，回到底部按钮出现。
3. 点击“回到底部”后重新显示段落 71–80。

证据：

- [初始底部画面](artifacts/before-scroll.png)
- [向上滚动画面](artifacts/scrolled-up.png)
- [回到底部画面](artifacts/back-to-bottom.png)
- [长消息窗口控件记录](artifacts/long-message-window.txt)
- [向上滚动控件记录](artifacts/scrolled-up-window.txt)
- [回到底部控件记录](artifacts/back-to-bottom-window.txt)

构建与预览：

```bash
cd /Users/roylin/code/aster/apps/studio
ASTER_TENANT_SLUG=aster STUDIO_RENDERER_DIST=.studio/dist/message-scroll-performance \
  node node_modules/@rsbuild/core/bin/rsbuild.js build

STUDIO_RENDERER_DIST=.studio/dist/message-scroll-performance \
  node node_modules/@rsbuild/core/bin/rsbuild.js preview \
    --host 127.0.0.1 --port 44363 \
    --dist-path .studio/dist/message-scroll-performance

cd /Users/roylin/code/aster
ASTER_DEFER_RUNTIME_COMPONENTS=1 just build-studio
```

## Explicitly not claimed

- 不宣称基于帧率或耗时基准的性能百分比提升；本证据减少的是可计数的无效渲染、索引计算和缓存写入。
- 不宣称完整发布包或可选沙箱组件通过验证；真实窗口使用延迟组件构建。
- 不宣称全量类型检查、整个仓库测试或所有 GUI 流程通过。
- 不使用用户真实会话做性能验证；长消息来自隔离的确定性测试任务。
