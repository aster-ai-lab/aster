# Aster 桌面端多语言更新说明规范

## 发布契约

每次正式桌面发布必须同时提供英文和简体中文说明。GitHub 工作流输入为 `notes_en` 与 `notes_zh_cn`，两者都是必填项；缺少任一语言时不得发布。

输入文件只写更新内容，不手写 `LANG` 注释。`scripts/desktop_release_manifest.py` 在生成 `desktop-latest.json` 时统一添加语言标记，避免标记顺序、拼写和配对不一致。

每条说明按“新功能、改进、错误修复”分组，每条变更单独一个 Markdown 列表项，并保留事实、顺序、单位和版本号：

```markdown
## New Features
- Added …
## Improvements
- Improved …
## Bug Fixes
- Fixed …
```

```markdown
## 新功能
- 新增……
## 改进
- 改进……
## 错误修复
- 修复……
```

生成器写入以下结构，最新说明同时保存在 `notes` 和 `history[0].notes`，历史版本不得重复或乱序：

```text
<!--LANG:en-->
English notes
<!--LANG:zh-CN-->
中文说明
<!--LANG:END-->
```

清单中的任意语言标记都必须成对、完整且内容非空。旧版没有语言标记的单语言说明保持兼容，不要求回写历史 Release。

## 客户端展示

- Aster 使用当前界面语言选择对应内容；中文显示简体中文，其他语言使用英文。
- 更新标题、按钮和分类名称使用界面语言，更新内容使用同一语言，不拼接两种语言。
- 跨版本更新只包含 `当前版本 < 说明版本 <= 目标版本` 的记录，每条变更保留其来源版本号。
- 没有匹配语言时回退到英文；旧版无标记说明按原文显示；标记不完整时发布检查必须失败，不能混合显示。

## 发布前检查

```sh
python3 tests/desktop_channel_manifest.py
python3 scripts/desktop_release_manifest.py check --manifest dist/desktop-latest.json
```

发布工作流必须依次完成清单生成和 `check`，通过后才能上传 `desktop-latest.json`。
