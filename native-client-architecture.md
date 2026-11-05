# Rust 原生客户端技术架构

> 目标：在体积最小化与跨平台兼容性最大化之间取得可交付的平衡，并覆盖 Windows、macOS、主流 Linux，以及麒麟 / UOS 等国产桌面环境。
>
> 非目标：默认路径不追求 Electron / 完整 Chromium 级网页兼容；不把自带浏览器内核作为主方案。

## 1. 设计原则

| 原则 | 含义 |
|------|------|
| 不自带浏览器内核 | UI 默认走原生控件或系统 WebView；Chromium / CEF / Ultralight 不进入默认依赖 |
| 业务与壳分离 | `client-core` 无窗口、无 GPU；平台壳可替换 |
| 特性可裁剪 | Cargo feature 控制 UI 后端、GPU、更新器；默认配置保持最瘦 |
| 依赖可预测 | 业务逻辑纯 Rust；动态依赖优先使用系统已提供的库 |
| 信创友好 | Linux 以 glibc 发行版为主路径；安装包声明明确的系统依赖 |

### 1.1 体积预期（数量级）

| 形态 | 量级（strip + LTO，粗算） |
|------|---------------------------|
| 无 UI 逻辑库 / headless | 很小 |
| 纯原生 GUI 壳 | 约数 MB～十余 MB |
| 系统 WebView 壳 | 安装包小，内核由系统提供 |
| 自带 HTML 引擎 | 数十 MB 起，仅作可选实验线 |

## 2. 总体结构

```text
┌─────────────────────────────────────────────┐
│  apps/<product>-desktop  （薄壳，按平台编译）  │
│  窗口 / 托盘 / 更新 / 协议注册 / 权限对话框    │
└─────────────────┬───────────────────────────┘
                  │ 同进程调用（优先）或薄 IPC
┌─────────────────▼───────────────────────────┐
│  crates/client-ui   UI 适配层（可替换后端）   │
│  Native  |  SystemWebView  |  (可选) HtmlLite│
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│  crates/client-core   纯业务（无窗口依赖）    │
│  会话 / 配置 / 同步 / 领域模型 / 错误类型      │
└───────┬─────────────────────────┬───────────┘
        │                         │
┌───────▼────────┐       ┌────────▼────────────┐
│ client-net     │       │ client-storage      │
│ reqwest+rustls │       │ sqlite              │
└────────────────┘       └─────────────────────┘
        │
┌───────▼─────────────────────────────────────┐
│  crates/client-platform  OS 抽象             │
│  路径 / 通知 / 钥匙串 / 开机启动 / 系统代理    │
└─────────────────────────────────────────────┘
```

**同进程优先**：避免再引入一套脚本运行时与重 IPC，减小体积与故障面。

## 3. UI 策略

采用「原生优先，系统 WebView 可选，自带引擎默认关闭」。

### 3.1 默认：Rust 原生 UI

- 候选：Slint、iced、egui 等（按观感与团队熟悉度择一，避免多套并存）。
- 图形：`wgpu` 作为可选 feature；在麒麟 / 弱 GPU 环境提供 CPU / 软件回退。
- 适用：工具型客户端、可控界面、离线与信创优先场景。
- 不适用：大量不可控的第三方网页、运营活动页硬嵌。

### 3.2 次选：系统 WebView

使用 `tao` + `wry`（可与 Tauri 同栈，但不强制引入完整 Tauri 应用框架）：

| 平台 | 后端 |
|------|------|
| Windows | WebView2（缺失时引导安装或降级到原生精简 UI） |
| macOS | WKWebView |
| Linux / 麒麟 / UOS | WebKitGTK（系统包，如 `webkit2gtk`） |

- 优点：安装包不携带浏览器内核，体积小；国产发行版适配通常优于自带 Chromium。
- 代价：渲染细节随系统 WebKit / WebView2 版本变化；需启动探测与降级路径。
- 前端资源：静态 HTML/CSS/JS；避免沉重 SPA 框架进入默认包。

### 3.3 默认不做

- Electron / CEF / 整包 Chromium。
- Ultralight 或自研迷你 WebKit（可单开实验线，不进默认 feature）。
- 以 txiki.js 等完整 JS runtime 充当 UI 内核（职责越界且体积与能力面过大）。

若页面仅需少量脚本：优先业务在 Rust；必要时仅嵌入 **QuickJS-ng**，而不是整包 JS runtime。

## 4. Crate 分层

| Crate | 职责 |
|-------|------|
| `client-core` | 异步业务、状态机、纯逻辑；依赖 tokio |
| `client-api` | 对 UI 稳定的命令 / 查询 / 事件面 |
| `client-platform` | `cfg(target_os)` 封装与可选发行版探测 |
| `client-net` | HTTP(S)；默认 rustls |
| `client-storage` | SQLite 单文件库与版本化迁移 |
| `client-ui-native` | feature `ui-native` |
| `client-ui-webview` | feature `ui-webview` |
| `client-updater` | 可选差分更新 |

### 4.1 建议 Cargo features

```toml
[features]
default = ["ui-native", "rustls"]
ui-native = []
ui-webview = []
gpu = []
updater = []
# 不在 default 中启用自带 HTML 引擎或 Chromium 系依赖
```

同一 `client-core` 可由 CI 产出：`native`、`webview`、`headless` 三种产物。

## 5. 仓库目录草图

```text
crates/
  client-core/
  client-api/
  client-platform/
  client-net/
  client-storage/
  client-ui-native/
  client-ui-webview/
apps/
  client-desktop/           # 唯一桌面入口，feature 选择 UI 后端
  client-desktop/dist/      # msix / dmg / deb（含麒麟）打包脚本
```

（具体 crate 名可按产品命名替换；分层与依赖方向保持不变。）

## 6. 跨平台兼容矩阵

| 层级 | 约束 |
|------|------|
| 目标三元组 | `x86_64-pc-windows-msvc`；`x86_64-apple-darwin` / `aarch64-apple-darwin`；`x86_64-unknown-linux-gnu`；麒麟按需增加 `aarch64-unknown-linux-gnu` |
| libc | Linux 主路径保证 **glibc**；不以 musl 作为信创默认 |
| TLS | 统一 **rustls** + 明确根证书策略，避免各发行版 OpenSSL 差异 |
| 路径与进程 | `Path` / `OsStr`；不假设 Bash；子进程走 `std::process` |
| 字体 | 原生 UI 优先系统字体；自绘 UI 按需嵌入中文字体子集 |
| 显示 | Linux 验证 X11 与 Wayland；WebView 依赖 WebKitGTK 显示后端 |
| GPU | `gpu` 可关闭；默认保证软件路径可启动 |

### 6.1 国产桌面交付

1. 提供 `.deb` 或厂商要求的包格式。
2. WebView 形态显式依赖系统 `webkit2gtk`（或等价包名）；Native 形态最小化动态依赖。
3. 启动时探测 WebView 可用性；失败则降级到 `ui-native` 精简界面。
4. 文档写清最低：glibc、WebKitGTK、显示协议、可选 GPU 要求。

## 7. 运行时模型

```text
main
 ├─ platform::init()           # 日志、单实例、可选崩溃上报
 ├─ core::runtime (tokio)      # 网络与磁盘，不阻塞 UI 线程
 ├─ ui::App::run()             # UI / 消息泵线程
 │    └─ 订阅 core 的 EventStream
 └─ graceful shutdown
```

- UI 线程与 tokio worker 分离。
- `client-api` 仅暴露：`Command`、`Query`、`Event`（实现可用枚举或紧凑序列化格式，择一并保持稳定）。
- 配置落在平台标准目录；密钥走 `client-platform` 的钥匙串抽象。

## 8. 体积治理

1. Release：`lto = fat`、`codegen-units = 1`、`strip = true`。
2. 默认 feature 保持最瘦；用 `cargo bloat` 等工具回归。
3. 依赖统一 `default-features = false` 并显式打开所需能力。
4. WebView 前端资源构建期打包并压缩；源映射不进发布包。
5. 更新通道使用差分包，避免「整包巨大运行时」式更新。
6. 按平台分别产物，不制作「三端合一」的胖包。

## 9. 推荐产品形态

### 形态 A — 极致体积与可控 UI（默认推荐）

`client-core` + `ui-native` + rustls + sqlite

适合工具客户端、离线、信创与长期维护。

### 形态 B — 需要 HTML 布局 / 运营页

`client-core` + `ui-webview`（wry/tao）+ 静态页面

安装包仍可保持较小；兼容性绑定系统 WebView；麒麟走系统包。

### 形态 C — 实验线（非默认）

自带轻量 HTML 引擎或精简 WebKit 系嵌入库：仅在「必须跨机像素级一致且不能依赖系统 WebView」时单独立项，不与默认架构混用同一 feature 集。

## 10. 与相关技术选型的边界

| 技术 | 在本架构中的位置 |
|------|------------------|
| 系统 WebView（wry） | 可选 UI 后端，网页方案的首选 |
| Slint / iced 等 | 默认原生 UI 后端候选 |
| QuickJS-ng | 可选的极薄页面脚本，非默认 |
| txiki.js | 可用作独立小 JS 运行时；不承担 HTML 渲染，不作为 UI 内核 |
| Ultralight / CEF / Electron | 非默认；体积与信创风险高 |
| TinyJS | 不推荐；语言能力过弱 |

## 11. 定案

**Rust 业务核心 + 可切换 UI 后端；默认原生；网页走系统 WebView；统一 rustls 与 SQLite；Linux 按 glibc 发行版适配国产桌面；任何自带浏览器内核均不进入默认路径。**

后续落地时，先固定唯一默认 UI 后端（原生或 WebView），再补齐打包流水线与目标麒麟版本的兼容性矩阵测试。
