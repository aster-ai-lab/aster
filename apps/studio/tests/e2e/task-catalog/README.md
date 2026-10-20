# `../tasks` 端到端用例

这是 `/Users/roylin/code/tasks` 的 AsterDream 用例带，和 `../tests` 的 22 项对齐分开。

`list.xlsx` 的「功能点」有 25 行。22 行有样例目录，3 行只有提示词（`CAT-MAIL`、`CAT-COPY`、`CAT-CONFLICT`）。同目录里的 `regex-log` 和 `git-leak-recovery` 只进入源文件哈希，仍走现有 reference-alignment 评分，不在这 25 条里再评一次。

## 这一步做了什么

冻结了清单、允许上传的文件和全目录 SHA-256。已经有核对答案的用例写成了评分器。评分器检查封闭事实，不比较整段措辞。

核对答案、`01_复制给模型的要求.txt` 和 A51 的 png 不会进入工作区。模型提示词写在作业根目录的 `instruction.txt`。

A06 和 A12 标成阻塞：对应 PDF 没有文字层，字段等现有 OCR 返回文本后再冻结。A01、A02、A04、A07、A08、A09、A10 的评分器也还没有冻结。空产物不会被算作通过。

## 命令

在本目录执行：

```sh
python3 pin.py
python3 grade_test.py
python3 run.py --case A23
```

`run.py --live` 会直接退出，不调用模型。

`window.py --launch --case A23` 用后台 CUA 打开 `just fork toyou`。`A3S_GUI_ALLOW_MOUSE` 必须保持未设置。点击、输入和热键只走 `cua_guard.GuardState`，投递模式是 `background`。这个命令不复制 `toyou/member-session.json`，也不结束已经在运行的 toyou。不带 `--launch` 时只核对内容门回执，不打开窗口。

A06 和 A12 仍然阻塞。空产物不会被算作通过。
