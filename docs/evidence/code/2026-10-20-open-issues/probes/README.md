# 真实 headless CLI 回归

`headless_cli.py` 用成对的真实 `aster`、`a3s-code-acp` 二进制和本机回环 provider 执行 bounded regression。每个子进程使用独立临时 HOME、GROK_HOME、XDG、ACL 与 workspace，不读取真实账号状态；单例上限 40 秒，超时只终止自有 process group。只记录合成标记与有界输出。

修复前以 `--phase before --binaries target/evaluations/open-issues-20261020/before-binaries` 执行两个认证场景；修复后以 `--phase after --binaries target/debug` 加测 CLI rules 与显式不支持的 replacement。都用 `--evidence docs/evidence/code/2026-10-20-open-issues`。二进制身份与每场结果写入对应 receipt，不能拿当前二进制冒充修复前。

这不是外部模型服务或真实账号测试。

第一次 after 四场景都通过，但 stub 只识别 native schema 名，未响应未知 OpenAI-compatible 端点使用的 prompt-mode pre-analysis，触发了本地 fallback。`headless-after-initial-*` 和 `headless_cli_initial.py` 保留这次结果。最终 fixture 按该测试唯一的 non-streaming pre-analysis 返回合法 JSON，并要求随后只有一次 main streaming request；这只修正 fixture，不改产品来迁就 stub。
