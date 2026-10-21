# AsterDream

租户标识为 `aster`，产品显示名为 AsterDream。它使用 AsterDream 宿主的同一套工作台；legacy 仍属于 `legacy` 租户。

品牌图形沿用 Work 的四角星芒、菱形星核、倾斜银色环轨和独立伴星。`logo.svg` 来自 `apps/work/logo.svg`，`app-icon.svg` 来自 `apps/work/build/app-icon.svg`，不重新绘制。资产源 SHA 记录于 `docs/evidence/desktop/2026-10-21-default-aster/brand-canonical-sources.json`。

| 文件 | 来源与用途 |
| --- | --- |
| `logo-light.png`、`logo-dark.png` | 原 Work `logo.png`，1024×1024 透明星标；保持原产品在两种表面的相同图形 |
| `mark-light.png`、`mark-dark.png` | 同一 `logo.svg` 的 256×256 透明导出；用于窗口、首页、加载页和对话标志 |
| `icon.png`、`icon.icns`、`icon.ico` | 原 Work `build/icon.*`，由 `app-icon.svg` 导出的桌面蓝底图标 |
| `tray.png` | 同一 `logo.svg` 的 66×66 透明导出；旧 Work 的 legacy 字样托盘图不沿用 |

主操作蓝为 `#005ABB`，悬停蓝为 `#004E9F`；工作台画布、导航和边框分别使用 `#F7F9FC`、`#E9EDF3`、`#DFE5ED`。深色主操作使用 `#4C9BE6`，内容表面保持深色。已保存的用户自定义主题色继续生效。

本租户不声明 `model_service`，不继承 ToGateway 名称或组织权限，不复制 Work 的认证、数据、模型配置或产品别名表。旧 Work 与 legacy 数据均保持原样。
