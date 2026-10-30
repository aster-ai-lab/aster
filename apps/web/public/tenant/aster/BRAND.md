# Aster

租户标识与构建选择保持 `aster`、`A3S_TENANT_SLUG=aster`，产品显示名统一为 Aster。
现有应用标识、数据目录与保存的用户主题继续兼容。

`logo.png` 是唯一品牌母版：曜石黑底、银白浮雕星芒与环轨、底部细颗粒纹理。
所有界面、官网、README、Dock 和任务栏使用同一图形，不保留版本后缀或独立绘制的大小图标。
macOS 状态栏使用同一星芒与环轨的透明底单色轮廓，交由系统模板染色。
不同尺寸和容器仅为平台所需的导出格式：

| 文件 | 用途 |
| --- | --- |
| `logo.png` | 唯一原始母版，1254×1254 PNG |
| `logo.svg` | 内嵌同图 1024 px 导出的官网兼容格式 |
| `icons/16x16.png` … `icons/1024x1024.png` | 尺寸阶梯，同时是 Linux hicolor 图标集 |
| `icon.png`、`icon.icns`、`icon.ico` | 石墨底桌面图标容器（png 1024 / icns / 多分辨率 ico） |
| `logo-light.png`、`logo-dark.png` | 同一张 512×512 图标，符合现有 512 KiB 租户资源上限 |
| `mark-light.png`、`mark-dark.png` | 256×256 窗口、首页、加载页和对话标志 |
| `tray-template.svg` | 为 16 pt 状态栏描绘的纯黑矢量轮廓，保留星芒与环轨 |
| `tray.png` | 32×32 透明单色状态栏模板，直接匹配 Retina 16 pt |
| `favicon.png` | 同一张图标的 32×32 浏览器导出 |
| `icon-assets.sha256` | 母版和全部位图、容器的生成校验记录 |

`just app-icons` 使用现有 Tauri CLI 从 `logo.png` 生成全部尺寸和平台容器，再生成 `logo.svg`。
生成文件不手工修改。明暗主题保留相同材质和颜色，不反色。

桌面和窗口使用带黑色底板的图标，Linux/Windows 托盘加载 `icons/32x32.png`。
macOS 从 `tray.png` 的 alpha 通道生成 16 pt（2× 为 32 px）单色模板，保留中空星核和环轨负空间，
由系统自动适配明暗背景及选中状态，避免把应用底板染成实心方块。

桌面身份的四处名称必须一致，否则坞站找不到入口、只能显示通用齿轮图标：

| 位置 | 取值 |
| --- | --- |
| `tauri.conf.json` 的 `identifier` | `dev.a3s.studio.aster` |
| 窗口类（X11 `WM_CLASS`、Wayland `app_id`） | `dev.a3s.studio.aster` |
| 桌面文件名 | `dev.a3s.studio.aster.desktop` |
| `Icon=` 与 hicolor 图标名 | `dev.a3s.studio.aster` |

主操作色为石墨黑 `#171717`，浅色悬停为 `#2A2A2A`；画布、导航与边框分别使用
`#F6F6F6`、`#E6E6E6`、`#E3E3E3`。深色主操作使用 `#F5F5F5`，文字与图标保持高对比。
应用图标通过曜石材质、银白星芒、环轨、高光和细颗粒形成层次；导航选中态沿用
ToWork 的中性灰底板，不用品牌色铺满。官网与应用使用同一图标，不另造图形。
用户显式选择的主题色和链接、成功、警告、错误等功能色不受默认黑白品牌约束。

本地构建不显示登录、用户资料或组织入口，不查询账号或组织接口。
另一个租户的账号、模型与组织行为继续独立，旧用户数据不迁移。
