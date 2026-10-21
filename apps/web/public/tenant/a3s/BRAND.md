# A3S OS

- 作者：RoyLin
- 日期：2026-09-30
- slug：`a3s`
- 目录：`apps/web/public/tenant/a3s/`
- 启动：`just up`、`just up a3s` 与 `just up:a3s` 都读取本目录

本目录是 A3S OS 这个租户的全部平台身份。旁边的 `INFO.md` 是加载器读取的记录。本文件给人读，加载器不解析它，也不把它算进重启用的哈希。

## 文件

| 文件 | 作用 |
| --- | --- |
| `BRAND.md` | 本租户的名称、字标和品牌色。产品控件用仓库根 `DESIGN.md` |
| `INFO.md` | 平台名称、logo 引用、主题色。加载器只读这一份 |
| `logo-light.png` | 亮色表面用的字标。透明底，墨色 `#171717` 圆环 A，1024×1024 |
| `logo-dark.png` | 暗色表面用的字标。透明底，浅色 `#F2F2F3` 圆环 A，1024×1024 |
| `icon.png` | 桌面应用图标。1024×1024，品牌蓝 `#2B4EDA` 底上的白色圆环标志，四边不透明并带 alpha |
| `icon.ico` | 同一枚图标的 Windows 文件。加载器不读 |
| `icon.icns` | 同一枚图标的 macOS 文件。加载器不读 |
| `mark-light.png` | 桌面窗口用的透明黑标。亮色表面 |
| `mark-dark.png` | 桌面窗口用的透明白标。暗色表面 |

文件名跟表面走：

- `logos.light_surface` = `logo-light.png`
- `logos.dark_surface` = `logo-dark.png`

控制台字标是开口圆环、无横杠的 A、底部一道波浪，透明底，墨色跟着表面走。亮色表面是 `#171717`，暗色表面是 `#F2F2F3`。PageLoading 用同一条轮廓先描边再填色。亮色加载底上是墨色，暗色加载底上是浅色。桌面图标把这枚白标放在 `#2B4EDA` 上，并由 `scripts/render-tenant-desktop-icons.py` 生成。加载器不读这些桌面文件。

## 平台名

中文名和英文名都是 A3S OS。桌面产品名是 A3S Desktop，写在 `INFO.md` 的 `product`。文档标题因此是 `A3S OS - 管理后台` 或 `A3S OS - 用户中心`，英文界面同样是 `A3S OS - Admin Console` 或 `A3S OS - User Center`。未登录的 `/` 只显示 A3S OS。

工作区 API 标题沿用现有表达式。名字以 ` OS` 结尾，所以中英文都显示为 `A3S Work API`。

## 颜色

| 字段 | 值 | 用在 |
| --- | --- | --- |
| `brand` | `#171717` | `--color-brand`。黑白主题色 |
| `on_brand` | `#FFFFFF` | `--color-on-brand` |
| `theme_color.light` | `#f6f6f6` | 浏览器 `theme-color`，亮色 |
| `theme_color.dark` | `#111112` | 浏览器 `theme-color`，暗色 |

主按钮保持黑色（暗色外观下为白），链接保持蓝色。`#171717` 不替换 `--color-primary`，也不铺进浏览器工具条。

## 控制台

`just up` 拉起的控制台用本目录的名称和字标：文档标题、favicon、登录字标、活动栏、侧栏品牌、门户和登录链接上的名称。设置里保存的名称或 logo 只盖住这一次启动的 `a3s`，颜色仍来自旁边的 `INFO.md`。

服务名和进程名不在本目录里，也不随本租户改变。它们使用 `a3s-` 前缀，例如 `a3s-api`、`a3s-gateway`、`a3s-runtime`。控制台若回显这些服务名，显示的是 `a3s-` 名字。

氪锶科技在 `apps/web/public/tenant/kense/`，用 `just up kense` 或 `just up:kense` 启动。通友科技在 `apps/web/public/tenant/legacy/`，用 `just up legacy` 或 `just up:legacy` 启动。
