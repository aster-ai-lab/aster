# Issue 7：workspace 门控提示

未能确认 `aster workspace` 功能开关时，明确说明 feature gate 未确认，并给出 `GROK_WORKSPACE_COMMAND=1` 的既有本地开启方式。不再把缺失设置归因为网络或登录问题。

`run_workspace_mgmt` 实际调用同一纯 `WorkspaceGate::require_enabled` 拒绝边界。保留环境覆盖优先、服务器开关、Unknown/Disabled 区分和失败退出语义；sandbox 的预先拒绝仍在原位置，未改变设置读取或网络策略。

## 回归

新增两项测试位于独立 sibling `apps/aster/src/workspace_command_gate_tests.rs`，以 `cfg(test)` 引入；原 main 内已有测试未搬迁。测试不读真实 HOME、登录或网络。

- 先作不改变行为的拒绝边界提取、保持旧文字，再运行新增回归：**1 失败、1 通过**。失败精确显示旧 network/login 诊断，证明它拒绝旧行为。[修复前日志](issue-7-before.log)、[命令与结果](issue-7-before.json)
- 仅替换 Unknown 文案后，新增两项及现有门控解析/环境解析共 **4/4 通过**。Cargo depfile 同时确认实际编译了新的 sibling 测试文件。[修复后日志](issue-7-after.log)、[命令与结果](issue-7-after.json)
- scoped `rustfmt --check`（不遍历或写子模块）和 `git diff --check` 通过，没有整文件格式化。

修改前已保存 main 的完整既有 diff 与 hash。对工作区 gate 所属区域之外逐字节归一比较相同，证明既有 updater 等内容未丢失；根 HEAD 仍为 `eb17c6ce5aa4c9ef785c12d56515e146866b1506`。[原差异](issue-7-main-before.patch)、[保留核对](issue-7-main-preservation.json)

本轮不启用隐藏命令、不改服务器授权、未验证真实远端账号或启动共享 leader。没有提交、推送，也未改已发布安装包、标签或 feed。
