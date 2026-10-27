# Aster

租户标识与构建选择保持 `aster`、`A3S_TENANT_SLUG=aster`，产品显示名统一为 Aster。
现有应用标识、数据目录与保存的用户主题继续兼容。

品牌图形由四角星芒、菱形星核、倾斜银色环轨和独立伴星构成。
`logo.svg` 与 `app-icon.svg` 是现有图形源；来源哈希保留于历史品牌证据。

应用图标按尺寸分档生成，不再是同一张图缩放到所有尺寸：

| 文件 | 用途 |
| --- | --- |
| `app-icon.svg` | 128 px 及以上母版：环轨双色渐变、板面高光、伴星齐全 |
| `app-icon-small.svg` | 48–80 px：加粗环轨、放大星体，去掉伴星 |
| `app-icon-micro.svg` | 16–40 px：只保留板面与星体，去掉环轨渐变与投影 |
| `icons/16x16.png` … `icons/1024x1024.png` | 尺寸阶梯，同时是 Linux hicolor 图标集 |
| `icon.png`、`icon.icns`、`icon.ico` | 石墨底桌面图标容器（png 1024 / icns / 多分辨率 ico） |
| `logo-light.png`、`logo-dark.png` | 1024×1024 透明石墨/银白星标 |
| `mark-light.png`、`mark-dark.png` | 256×256 窗口、首页、加载页和对话标志 |
| `tray.png` | 66×66 透明单色托盘标志（macOS 模板图来源；Linux/Windows 托盘用 `icons/32x32.png`） |

三个应用图标 SVG 由 `scripts/app-icon.py` 生成，`logo.svg` 是透明星标源；全部位图由
`scripts/app-icon-assets.py` 光栅化（`just app-icons` 一次跑完）。深色标志是浅色单色位图
保留 alpha 的精确反色，运行时暗色滤镜使用同一规则。板面是超椭圆而非圆角矩形：1024 画布内留 100 px
光学边距、板面 824 px，与 macOS 图标网格一致；低于 48 px 时圆角矩形会比平台遮罩
更方，超椭圆才不会。

桌面图标与窗口图标使用 `icon.png`，不用透明星标：任务栏把这张位图直接画进
22–48 px 的位置，透明图形在浅色面板上看起来像空位。

桌面身份的四处名称必须一致，否则坞站找不到入口、只能显示通用齿轮图标：

| 位置 | 取值 |
| --- | --- |
| `tauri.conf.json` 的 `identifier` | `dev.a3s.studio.aster` |
| 窗口类（X11 `WM_CLASS`、Wayland `app_id`） | `dev.a3s.studio.aster` |
| 桌面文件名 | `dev.a3s.studio.aster.desktop` |
| `Icon=` 与 hicolor 图标名 | `dev.a3s.studio.aster` |

主操作色为石墨黑 `#171717`，浅色悬停为 `#2A2A2A`；画布、导航与边框分别使用
`#F6F6F6`、`#E6E6E6`、`#E3E3E3`。深色主操作使用 `#F5F5F5`，文字与图标保持高对比。
应用图标通过石墨渐变、金属银星芒、环轨、高光和细描边形成材质层次；导航选中态沿用
ToWork 的中性灰底板，不用品牌色铺满。官网与应用使用相同的单色星标，不另造图形。
用户显式选择的主题色和链接、成功、警告、错误等功能色不受默认黑白品牌约束。

本地构建不显示登录、用户资料或组织入口，不查询账号或组织接口。
另一个租户的账号、模型与组织行为继续独立，旧用户数据不迁移。
