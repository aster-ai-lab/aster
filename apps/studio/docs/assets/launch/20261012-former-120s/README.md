# Former · 工作台漫游

120 秒，1920×1080，30 fps，H.264 + AAC。真实 Former 原生界面、中文说明、原创配乐。范围为任务、知识、智能体浏览、自动化配置；未打开或展示插件页面。

- 成片：`Former-120s-1080p.mp4`
- 封面：`poster.png`
- 可定位预览：`film.html?t=53`；连续预览：`film.html?play`
- 分镜与素材边界：`CONCEPT.md`
- 原始截图及对应 AX 状态：`capture/`
- 字体与工具许可：`FONT-LICENSES.txt`、`ONETAKE-LICENSE.txt`
- 原创声音说明：`SOUND-CREDITS.md`
- 检查记录：`composition-check.json`、`probe/`、`verification.log`、`QA.md`

所有产品内容来自真实界面。输入采用实际草稿区域的逐步揭示；镜头和说明由 onetake 合成。任务回答来自一次真实完成的演示请求。知识内容为已有测试笔记。自动化只到工作日 09:00 的配置草稿，拍摄后已取消；未保存或启动计划。智能体只展示市场说明，未声称安装成功或完成合同审查。

结尾图标与 `../../../../src/renderer/assets/images/logo.png` 字节一致。Former 黑白字标保留原样。

重现制作：在本目录运行 `prepare.py`、`score.py`，再使用已安装的 onetake `render.py film.html --out Former-120s-1080p.mp4 --sfx score.wav --workers 8 --gap 6 --samples-min 4 --samples-max 48`。渲染采用 180° 快门的多次采样；1080p 为待审版本。
