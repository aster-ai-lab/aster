# 0.2.1 安装后端到端真窗口验证（2026-11-09 18:2x）

用户要求「本机安装一下端到端测试一下」。从**修复后**的 `aster_0.2.1_aarch64.dmg`（SHA-256 `6aaca20ae4bcae31705b4204596e2172dd7f79445df054bb0e52f2fdc27c4d99`）安装到 `/Applications/Aster.app`，用 `open -g` 后台启动（**不激活、不抢焦点**），再用 `agent-desktop` 的 headless semantic 能力做真窗口验证。

## 结论：通过

| 检查 | 结果 |
| --- | --- |
| 安装 | `ditto` 到 `/Applications/Aster.app`（197M）；`CFBundleShortVersionString=0.2.1`，`identifier=dev.aster.studio.aster` |
| 签名/公证 | `codesign --verify --deep --strict` OK；`spctl` = `Notarized Developer ID` |
| 交付清单 | **100/100** 文件 SHA-256 与 size 匹配（即 §10 修复后的包） |
| 进程/窗口 | `list-windows` → `Aster` 1440×920，`visible=true`，**`is_focused=false`**（全程未抢焦点） |
| 主界面 | AX 38 个 ref：工作 / 设置 / 新建任务 / 搜索任务 / 连接器 / 技能 / 自动化 / 项目 / 最近任务、任务输入框、DeepSeek Flash、授权模式、任务范围。截图 `work-home.png` |
| 设置页 | 点击「设置」成功，元素 36 → 67，出现「模型配置」「用量」「搜索设置…」「返回应用」；截图 `settings.png`。设置侧栏无残留的本地路由 / 本地预算 tab |
| 搜索 | 点击「搜索任务」打开 dialog，含搜索输入框与「输入任务名称」空态；点「关闭」正常返回工作页 |
| 任务详情 | 点击已有任务「你有什么技能？」→ **521 个 AX 元素**，含用户消息与 11/02 18:45 时间、Aster 回复「已处理 14.3 秒」、复制/删除操作。截图 `task-conversation.png` |
| 错误态 | `find` 查 `Runtime plugin` / `pin identity` / `update_integrity_failed` **全部 0 命中**；无首次运行 pin 模态、无错误弹窗、无空白页 |

视觉复核：`work-home.png` 侧栏 + Aster 标题 + 输入框 + 模型选择完整；`settings.png` 分类导航（通用/快捷键/用量/模型服务/模型配置/网络搜索/文档处理/数据/归档/关于）与「通用」内容、开关、语言下拉完整。

## 边界（不假绿）

- `type` / `set-value` 对 Tauri WKWebView 的富文本任务输入框回报 `ACTION_FAILED` / `STALE_REF`：headless 语义输入无法给该控件设值。按照「不抢鼠标和窗口焦点」的约束，没有改用 `--headed` 物理输入，因此**未验证发送真实任务**，也未点击「发送」。页面切换、搜索空态、任务详情读取均已验证。
- 本机为 macOS 26.6.2 / arm64；包为 arm64 only，另一台机器的芯片与系统版本需另行核对（见 `../2026-11-09-release-0.2.1-attempt/OUTCOME.md` §11）。
- 本测试使用本机已有的用户数据（最近任务列表非空）；全新 HOME 的启动已在上一次用隔离 HOME 直接运行二进制验证过（20s 持续运行）。

## 复现命令要点

```bash
hdiutil attach -readonly -nobrowse dist/aster_0.2.1_aarch64.dmg
ditto /Volumes/Aster/Aster.app /Applications/Aster.app
hdiutil detach /Volumes/Aster -force
open -g -a /Applications/Aster.app          # -g = 后台，不激活
agent-desktop list-windows --app Aster       # 取 window-id
agent-desktop snapshot --window-id <ID> --max-depth 8 --compact
agent-desktop click <ref>                   # 语义点击，不使用 --headed / mouse-*
agent-desktop screenshot --window-id <ID> <path>
```
