# Aster

**把模型的建议，变成有边界、有记录、可验证的执行。**

Aster 提供终端编程工具 **Aster CLI** 和桌面工作台 **AsterDream**。
它们使用本仓库维护的 Rust Harness：连接模型、组织上下文、调用工具，并由宿主控制权限和完成条件。

[快速开始](#快速开始) · [安装、更新和卸载](#安装更新和卸载) · [理解 Meta Harness](#理解-meta-harness) · [开发桌面应用](#开发桌面应用) · [参与开发](#参与开发)

![Aster Meta Harness：装配工作台的底座固定，推演、执行、核验三个工位可以组合；所有工作遵守同一执行边界。](assets/readme/meta-harness.svg)

**读图：**固定底座代表内核边界；三个工位表示任务中的推演、执行和核验。
这是任务过程的概念示意，实际组件见下文。
[打开交互图源码](assets/readme/hairline-workbench.html)，下载后用浏览器打开，移动指针或按 **play** 查看三个工位。

## 从一个任务理解 Aster

假设你提出：“修复配置解析的问题，并运行相关测试。”

一个能交付这项工作的系统，需要把四件事接起来：

| 要解决的问题 | Aster 中的责任 |
| --- | --- |
| 接下来做什么？ | 模型根据任务和当前上下文提出下一步。 |
| 这一步允许执行吗？ | 宿主权限策略在工具调用前作出决定。 |
| 实际发生了什么？ | 工具结果和控制事件进入同一份事实日志。 |
| 结果达到完成条件了吗？ | 内核依据本次修改对应的验证证据判定是否可以完成。 |

因此，一次任务沿着这条路径推进：

```text
用户任务 → 模型提案 → 权限判定 → 工具执行 → 结果写入事实日志
                       │                         │
                       └─ 等待确认 / 拒绝         └─ 更新上下文，继续推演

发生修改的运行 → 与修改摘要绑定的验证报告 / 宿主显式豁免 → 完成门禁
```

模型、工具和界面可以替换。这些责任之间的边界由执行内核维持。

## 快速开始

需要 **rustup、C/C++ 工具链、CMake、protoc、just 和 Python 3.11+**。
Rust 版本由 [rust-toolchain.toml](rust-toolchain.toml) 固定；macOS 还需要 Xcode Command Line Tools。
Linux 桌面依赖见 [CI 安装步骤](.github/workflows/ci.yml)。
只安装 `aster` 和 `a3s-code-acp` 时，用[安装、更新和卸载](#安装更新和卸载)里的脚本地址，不必先进入仓库。

```sh
git clone https://github.com/aster-ai-lab/aster.git
cd aster
```

先将模型配置写入 `~/.a3s/config.acl`。以下是 OpenAI 兼容接口的最小示例；
将接口地址和 `model-id` 替换为你的服务信息，并在本机环境中设置 `OPENAI_API_KEY`：

```acl
default_model = "openai/model-id"

providers "openai" {
  base_url = "https://your-provider.example/v1"
  api_key = env("OPENAI_API_KEY")
  models "model-id" {}
}
```

更多选项见 [ACL 配置示例](crates/code/agent.example.acl)。
也可以用 `A3S_CONFIG` 指定配置文件，在终端中通过 `/config` 管理模型。
密钥留在本机配置或环境中；工作区配置受信任边界约束。

```sh
just code
just code --cwd /path/to/project
```

`just code` 先编译 `aster` 和 `a3s-code-acp`，再启动终端。
启动屏使用缓存的星芒轨道 ASCII 动画，并适配较小的终端。

> **macOS 当前限制：**原生 Shell 工具被上游进程生命周期保护拒绝，错误为
> `native_process_lifetime_unqualified`。界面、模型对话和文件工具已有本地验证；
> 依赖 Shell 的完整编程流程仍需通过平台资格验证。详见 [验证记录](docs/validation.md)。

<details>
<summary>直接使用 Cargo、发布模式和本地安装</summary>

默认成员会同时构建 CLI 和 ACP。两个可执行文件需要放在同一目录；
`A3S_ACP_AGENT_BIN` 可以显式指定 ACP 路径。支持 `CARGO_TARGET_DIR`。

```sh
cargo build --locked
./target/debug/aster --cwd /path/to/project

cargo build --locked --release
./target/release/aster

cargo install --locked --path crates/code/acp
cargo install --locked --path apps/aster
aster
```

把程序安装到系统目录、以后更新和卸载，见 [安装、更新和卸载](#安装更新和卸载)。

仅运行 `cargo run -p aster` 时，需要先构建 ACP。
源码版本通过重新构建更新，上游二进制自动更新已停用。

</details>

## 安装、更新和卸载

安装要解决的是：在本机得到同一目录里的 `aster` 和 `a3s-code-acp`，并且之后能从终端启动。
`aster` 默认启动旁边的 `a3s-code-acp`。需要别的路径时，设置 `A3S_ACP_AGENT_BIN`。

安装命令下载 [aster-cli v0.1.1](https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.1.1) 的发布包，并把包里的两个程序放到同一目录。
它不克隆仓库，也不在本机编译。
它不下载上游 CLI，也不打开自动更新。
macOS 和 Linux 需要 `curl`。Windows 使用 PowerShell。安装程序不修改 shell 配置，因此还要把目标目录放进 `PATH`。

`just code` 只在仓库里启动调试构建，不把程序复制到系统目录。

`aster update` 下载本仓库的 [aster-cli v0.1.1](https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.1.1) 归档，并替换所调用 `aster` 所在目录里的 `aster` 和 `a3s-code-acp`。
这个发布包里是这两个程序。它们不提供 `--build-id`。归档另外同时带有 `BUILD_ID` 和 `SHA256SUMS` 时，两个程序必须带有同一个 `aster-cli-pair-v1` 构建身份。
下载失败，或带身份文件时两个程序的身份不一致，原来的两个程序保持原字节并且仍可运行。
下载在终端中进行，且输出不是 JSON 时，画面是 ascii.rest 的 [`progress-bar`](https://ascii.rest/progress-bar/)。填充随收到的字节增加，更新结束时动画停止。
`aster update --check` 和任何带 `--json` 的状态只打印结果，不替换文件，输出里也没有这段动画。
目录里已经是该发布包时，`aster update` 不改这两个文件的字节，也不报告安装完成。
`~/.a3s` 和 `~/.grok` 保持不变。普通启动、leader 和 stdio 不会在后台自动更新。

也可以再执行一次下面该系统的安装命令。
安装命令下载同一个 v0.1.1 发布包，只覆盖安装目录里的这两个程序。
然后执行 `aster version --json`，确认 `currentVersion` 以 `0.1.1` 开头。

卸载只删除这两个程序。`~/.a3s` 和 `~/.grok` 都保留。
装到其他目录时，更新和卸载都针对那个目录里的同名文件。

| 系统 | 发布包 | 默认目录 | 程序名 |
| --- | --- | --- | --- |
| macOS | https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-macos.tar.gz | `$HOME/.local/bin` | `aster`、`a3s-code-acp` |
| Linux | https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-linux.tar.gz | `$HOME/.local/bin` | `aster`、`a3s-code-acp` |
| Windows | https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v0.1.1/aster-cli-v0.1.1-windows.zip | `%LOCALAPPDATA%\Aster\bin` | `aster.exe`、`a3s-code-acp.exe` |

安装后的程序就是该发布包里的程序。macOS 上原生 Shell 工具仍会被拒绝，见[快速开始](#快速开始)中的限制。

### macOS

安装命令下载上表中的 macOS 发布包 `aster-cli-v0.1.1-macos.tar.gz`。

```sh
curl -fsSL https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh | bash -s -- --prefix "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
aster version --json
a3s-code-acp --version
```

`export` 只作用于当前终端。要长期生效，把同一行写入 shell 的启动文件。
`product` 是 `Aster`。`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。`a3s-code-acp --version` 打印 ACP 的包版本。

更新再执行一次 macOS 安装命令。它下载 v0.1.1 的 macOS 发布包，只覆盖 `$HOME/.local/bin` 里的 `aster` 和 `a3s-code-acp`。`~/.a3s` 保持不变。

```sh
curl -fsSL https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh | bash -s -- --prefix "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
aster version --json
```

`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。

卸载：

```sh
rm -f "$HOME/.local/bin/aster" "$HOME/.local/bin/a3s-code-acp"
```

### Linux

安装命令下载上表中的 Linux 发布包 `aster-cli-v0.1.1-linux.tar.gz`。脚本按本机系统选择发布包。

```sh
curl -fsSL https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh | bash -s -- --prefix "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
aster version --json
a3s-code-acp --version
```

`export` 只作用于当前终端。要长期生效，把同一行写入 shell 的启动文件。
`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。

更新再执行一次 Linux 安装命令。它下载 v0.1.1 的 Linux 发布包，只覆盖 `$HOME/.local/bin` 里的 `aster` 和 `a3s-code-acp`。`~/.a3s` 保持不变。

```sh
curl -fsSL https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.sh | bash -s -- --prefix "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
aster version --json
```

`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。

卸载：

```sh
rm -f "$HOME/.local/bin/aster" "$HOME/.local/bin/a3s-code-acp"
```

### Windows

在 PowerShell 中，第一条命令保存安装脚本，第二条下载上表中的 Windows 发布包 `aster-cli-v0.1.1-windows.zip` 并复制两个程序。

```powershell
irm "https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.ps1" -OutFile "$env:TEMP\install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\install.ps1" -Prefix "$env:LOCALAPPDATA\Aster\bin"
$env:Path = "$env:LOCALAPPDATA\Aster\bin;" + $env:Path
aster version --json
a3s-code-acp --version
```

`Path` 赋值只作用于当前窗口。Git Bash 使用 macOS 的同一条命令；脚本在 Windows 上下载 Windows 发布包。
`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。

更新再执行一次 Windows 安装命令。它下载 v0.1.1 的 Windows 发布包，只覆盖 `%LOCALAPPDATA%\Aster\bin` 里的 `aster.exe` 和 `a3s-code-acp.exe`。`~/.a3s` 保持不变。

```powershell
irm "https://raw.githubusercontent.com/aster-ai-lab/aster/main/scripts/install.ps1" -OutFile "$env:TEMP\install.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -File "$env:TEMP\install.ps1" -Prefix "$env:LOCALAPPDATA\Aster\bin"
$env:Path = "$env:LOCALAPPDATA\Aster\bin;" + $env:Path
aster version --json
```

`aster version --json` 确认 `currentVersion` 以 `0.1.1` 开头。

卸载：

```powershell
Remove-Item -Force "$env:LOCALAPPDATA\Aster\bin\aster.exe", "$env:LOCALAPPDATA\Aster\bin\a3s-code-acp.exe"
```

## 理解 Meta Harness

**Harness 把一次模型调用接入可执行的任务过程；Meta Harness 让宿主在同一内核上组合这个过程。**

这里的 **Meta** 指 Harness 本身可以被装配：宿主选择部件和扩展组件来组织任务行为。
每个组件只根据事实日志计算当前视图和可执行转换。进程内存里的等待表不参与恢复。
运行时执行一个已启用的转换，再把结果追加为新事实，然后重新计算：

```text
状态 = 按顺序折叠事实日志
视图、可执行转换 = 组件树(状态)
执行转换 → 追加结果事实 → 再次计算
```

这对应源码里的 Moore 组件接口：`initial / step / output`。
每个转换有唯一键；原因已经写入日志的转换不会再次调度。
确认与提问停住，直到对应的答案事实写入。计时器不能代替这个答案。
实现见 [Effect 运行时](crates/effect/src/actor.rs) 和 [Core 组合入口](crates/code/core/src/meta_harness.rs)。

### 一步怎么留下记录

用户文字先成为 `user.message`。调度器只读日志。下一步是推理、工具、压缩、等待确认，或结束。

工具调用先追加准备事实，执行结束后再追加收据。进程若停在这两笔事实之间，恢复不把该工具再执行一次，只补一条未知结果。键和载荷都相同的再次写入保持原样。键相同而载荷不同则拒绝写入。追加之后日志落盘。

压缩把较早的上下文收成一条摘要事实。下一次推理读摘要和保留的尾部。

### 谁有权做决定

| 决定 | 权威 | 不由谁决定 |
| --- | --- | --- |
| 这次运行的历史 | 事实日志 | 运行存储和界面事件只是投影 |
| 工具能不能执行 | 本次运行准入时冻结的权限策略 | 后到的姿态不改写已冻结的工具名单 |
| 运行能不能完成 | 完成门禁 | 模型正文不是证据 |
| 命令在什么边界里跑 | 沙箱 | `bash` 不直接生成宿主进程 |

权限按 Deny、Allow、Ask 的顺序匹配。壳命令先做一次解析分类，策略再读这个分类。子运行继承工作策略，只能收窄权限，不能自带目标。

没有文件修改时，最终回答可以结束这次运行。有修改时，验证报告或宿主显式豁免必须绑定同一次效果摘要。未收回的工具效应，或尚未观察的后台写入，都不能算完成。`GoalAchieved` 只在完成门禁允许之后发出。

宿主组件不能禁用权限检查，也不能禁用完成门禁。宿主可以提交一份绑定效果摘要的通过报告。那份报告是证据，不是绕过。运行返回成功之前要完成资源作用域的清理。清理失败则拒绝成功。墙钟、工具回合、模型重试和上下文长度各有自己的上限。

工作策略和长期目标记在同一份日志里。目标按原有 Run 分段推进，受宿主预算和等待条件限制。细节见 [持续工程与目标](crates/code/manual/DURABLE_ENGINEERING.md)。事实重放恢复的是控制状态。外部副作用的崩溃恢复要另行验证。记忆仓库负责隔离和修订，不判断一句话是否为真。未激活的候选不进入提示。边界见 [持久记忆](crates/code/manual/DURABLE_MEMORY.md)。

### 可以组合什么

默认组合由五个部件构成。省略自定义配置时，会话继续使用默认 `coding_actor`：

| 部件 | 职责 |
| --- | --- |
| `system` | 系统指令槽。 |
| `tools` | 工具目录槽。 |
| `budget` | 工具调用预算。 |
| `compact` | 上下文压缩阈值。 |
| `infer` | 驱动模型、工具和等待状态的调度子树。 |

Rust 宿主还可以通过 `HostHarnessRegistry` 注册 `host:<id>` 组件，或用 `HostHarnessAssembler` 装配组件树。
例如，为一个会话选择部件、设置 8 次工具调用预算和 32,000 字符的上下文压缩阈值：

```rust
use a3s_code_core::{HarnessComposeOptions, SessionOptions};

fn session_options() -> anyhow::Result<SessionOptions> {
    let harness = HarnessComposeOptions::compose(
        vec![
            "system".into(),
            "tools".into(),
            "budget".into(),
            "compact".into(),
            "infer".into(),
        ],
        Some(8),
        Some(32_000),
        vec!["完成修改后，使用相关测试验证结果。".into()],
    )?;
    Ok(SessionOptions::default().with_harness(harness))
}
```

这是 Rust 嵌入接口。CLI 使用产品默认组合。完整契约见 [Meta Harness 手册](crates/code/manual/META_HARNESS.md)。

## 从哪里读源码

```text
Aster CLI / AsterDream
          │ ACP，标准输入输出
          ▼
     伴随进程
          │
          ▼
   Meta Harness · 会话、工具、完成门禁
          │
          ▼
   Effect · 组件与事实日志
```

这张图是两个产品的智能体会话路径。桌面窗口、文档和系统集成还有各自的宿主代码。

普通交互会话由宿主启动旁边的伴随进程。找不到该进程时，启动失败，不改用进程内代理。带 `--leader` 的终端会话仍使用进程内代理，工具表与伴随进程不同。

工作区路径是模型工具的根目录。它不限制伴随进程读取本机配置，也不限制会话上另行启动的 MCP 进程。产品会话的模型回合只走这份事实日志。仓库里仍留有旧的独立循环。新运行不走它，旧入口只减少。

| 你要修改的行为 | 先读这里 |
| --- | --- |
| CLI 启动、参数与伴随进程 | [apps/aster](apps/aster) |
| 终端交互、欢迎屏与渲染 | [crates/terminal](crates/terminal) |
| ACP 会话与产品工程策略 | [crates/code/acp](crates/code/acp) |
| Harness 组合、权限与完成门禁 | [crates/code/core](crates/code/core) |
| 组件投影、效果执行和事实重放 | [crates/effect](crates/effect) |
| 扩展能力、检索与持久化记忆 | [Use](crates/use)、[Search](crates/search)、[Memory](crates/memory)、[Vec](crates/vec) |
| AsterDream / toyou 共享桌面宿主 | [apps/studio](apps/studio) |
| AsterDream 旧版源码与 Office vendor 依赖 | [apps/work](apps/work) |
| Office 引擎与编辑器 | [crates/office](crates/office)、[Office 编辑器源码](apps/work/vendor/packages/office) |

所有 A3S Rust 依赖随仓库维护，成员均为 `publish = false`，共享根目录工作区和 `Cargo.lock`。
构建 Aster 和 Aster Work 不需要先发布独立的 Harness 包，也不需要相邻的 `../a3s` 仓库。
Cargo 和 pnpm 仍会下载普通第三方依赖。
Code 只保留 Rust SDK；内部 `a3s-*` 包名用于保留现有 Rust API 兼容性。

## 开发桌面应用

桌面应用统一使用 `apps/studio` 的 Rust 宿主和前端。默认 `aster` 租户为 AsterDream，
保留星标和蓝银主题；`toyou` 租户保留原有通友品牌。
需要 Node 和 pnpm 12.6.0；Node 版本范围见 [package.json](apps/studio/package.json)。

```sh
cd apps/studio
pnpm install --frozen-lockfile
cd ../..
just build-studio
just studio
# 原 Studio / ToWork 产品
just studio toyou
```

`just build-studio` 先构建 ACP、Office，再构建共享宿主。
`just dev`、`just work` 和 `just studio` 使用 AsterDream 开发入口，默认租户为 `aster`（AsterDream），
前端热更新端口为 `4318`。macOS 安装包：

```sh
just build-studio-release
# 显式生成 ToWork 包
A3S_TENANT_SLUG=toyou just build-studio-release
```

新 AsterDream 使用独立应用标识和新数据目录，本次合并不读取或迁移旧版会话、项目和凭据。
`apps/work` 和 `just build-work` 保留为旧版源码与构建入口；默认启动不再终止旧版进程。
Office 编辑器和 `a3s-office` 仍从本仓库的 [crates/office](crates/office) 构建，
Office 内核构建保留现有 vendor 路径。说明见 [Office 编辑器](apps/work/vendor/packages/office/README.md)。
范围与来源见 [AsterDream 迁移记录](docs/migration.md#studio-import) 和
[默认 AsterDream 租户规格](specs/004-studio-default-aster/spec.md)。

## 参与开发

先找到行为的拥有者和调用方，再修改最小的完整路径。保持权限、配置的信任边界和完成门禁；
用工具结果、文件内容或状态断言验证行为。仓库约定见 [Agents.md](Agents.md)。
工程规则、宪法和 spec-kit 阶段见 [开发规范](docs/development-standards.md)。

从根目录运行与变更对应的检查：

```sh
python3 tests/workspace_dependencies.py
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked -p a3s-code-core --lib meta_harness
cargo test --locked -p a3s-effect
cargo test --locked -p aster
```

- [CI](https://github.com/aster-ai-lab/aster/actions/workflows/ci.yml)：查看当前分支的实际检查结果。
- [本地验证记录](docs/validation.md)：已验证的范围、macOS 限制及 旧版桌面应用尚未通过的完整质量检查。
- [迁移与来源](docs/migration.md)：依赖内化、兼容性选择和来源清单。
- [README 图形源码与重建](assets/readme/README.md)：Hairline 工作台的交互页与静态预览。
- [问题与缺陷](https://github.com/aster-ai-lab/aster/issues)：报告时附操作系统、复现步骤、预期结果和去除凭据的日志。

上游手册保留历史架构资料，其中独立发布和其他语言 SDK 的说明不适用于本仓库。

## 许可证

Aster 入口和仓库根许可证是 [MIT](LICENSE)。终端树保留 [Apache-2.0](crates/terminal/LICENSE)。Code / Effect 的许可见 [MIT](crates/code/LICENSE) 与 [Effect 许可](crates/effect/LICENSE)。
桌面组件保留 [AGPL-3.0](apps/studio/LICENSE)，旧版组件许可见 [旧版许可证](apps/work/LICENSE)。分发时请按组件保留原有许可证和第三方声明。
Hairline 图形引擎的 MIT 许可证随[图形资产](assets/readme/hairline-license.txt)保留。
