# 氪锶科技

- 作者：RoyLin
- 日期：2026-09-29
- slug：`kense`
- 目录：`apps/web/public/tenant/kense/`
- 启动：`just up kense` 与 `just up:kense` 读取本目录。`just up` 读取 `tenant/a3s`

本目录是 Kense 这个租户的全部平台身份。旁边的 `INFO.md` 是加载器读取的记录。本文件给人读，加载器不解析它，也不把它算进重启用的哈希。

## 文件

| 文件 | 作用 |
| --- | --- |
| `BRAND.md` | 本租户的名称、字标和品牌色。产品控件用仓库根 `DESIGN.md` |
| `INFO.md` | 平台名称、logo 引用、主题色。加载器只读这一份 |
| `logo-light.png` | 亮色表面用的字标。白底黑 K，180×180 |
| `logo-dark.png` | 暗色表面用的字标。透明底白 K，蓝色点缀 `#4e60ff`，180×180 |
| `icon.png` | 桌面应用图标。1024×1024，白底上的 K，四边不透明并带 alpha。标志约占画布宽度的 62% |
| `icon.ico` | 同一枚图标的 Windows 文件。加载器不读 |
| `icon.icns` | 同一枚图标的 macOS 文件。加载器不读 |
| `mark-light.png` | 桌面窗口用的透明黑 K。亮色表面 |
| `mark-dark.png` | 桌面窗口用的透明白 K。暗色表面 |

文件名跟表面走：

- `logos.light_surface` = `logo-light.png`
- `logos.dark_surface` = `logo-dark.png`

## 平台名

中文名是氪锶科技，英文名是 Kense OS。桌面产品名是 Kense Work，写在 `INFO.md` 的 `product`。文档标题因此是 `氪锶科技 - 管理后台` 或 `氪锶科技 - 用户中心`，英文界面是 `Kense OS - Admin Console` 或 `Kense OS - User Center`。未登录的 `/` 只显示当前语言的平台名。字标上的字母仍是 Kense。

工作区 API 标题沿用现有表达式。中文名不以 ` OS` 结尾，显示为 `氪锶科技 Work API`。英文名 `Kense OS` 会剥掉末尾的 ` OS`，显示为 `Kense Work API`。`Kense Work` 仍是 Work 产品名。

## 颜色

| 字段 | 值 | 用在 |
| --- | --- | --- |
| `brand` | `#171717` | `--color-brand`。黑白主题色 |
| `on_brand` | `#FFFFFF` | `--color-on-brand` |
| `theme_color.light` | `#f6f6f6` | 浏览器 `theme-color`，亮色 |
| `theme_color.dark` | `#111112` | 浏览器 `theme-color`，暗色 |

主按钮保持黑色（暗色外观下为白），链接保持蓝色。`#171717` 不替换 `--color-primary`。

## 控制台

`just up:kense` 拉起的控制台用本目录的名称和字标：文档标题、favicon、登录字标、活动栏、侧栏品牌、门户和登录链接上的名称。设置里保存的名称或 logo 只盖住这一次启动的 `kense`，颜色仍来自旁边的 `INFO.md`。PageLoading 描的是这枚 K：先描边，再填成墨色。

服务名和进程名不在本目录里，也不随本租户改变。它们使用 `a3s-` 前缀，例如 `a3s-api`、`a3s-gateway`、`a3s-runtime`。控制台若回显这些服务名，显示的是 `a3s-` 名字。

默认租户在 `apps/web/public/tenant/a3s/`。`just up` 读取那个目录。通友科技在 `apps/web/public/tenant/legacy/`，用 `just up legacy` 启动。桌面图标和窗口字标由 `scripts/render-tenant-desktop-icons.py` 从上面的两张字标生成。
