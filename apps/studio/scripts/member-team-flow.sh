#!/usr/bin/env bash
# 两人团队故事的 ToWork 窗口车道。
# 不启动 Studio，不读取、不复制会员会话。
# 甲应当看到：邀请、新建职位，以及成员行上的角色和职位。角色目录只有甲能改，内置角色的勾选是锁住的。
# 乙应当看到邀请，不应当看到新建角色、解散团队、部门编辑。
# 按钮按权限规则判断。今天乙的邀请按钮，以及其他只跟着管理员标记走的按钮，会失败，失败要留下来。
set -euo pipefail

if ps -ax -o command= | grep -E '/dev\.a3s\.studio/.*/Contents/MacOS/' >/dev/null; then
  echo "窗口场景未运行：已有 debug Studio。再开一套会关掉正在使用的 ToWork。" >&2
  exit 2
fi

if [[ -z "${FLOW_HOME:-}" || -z "${A3S_ORG_API_ORIGIN:-}" ]]; then
  echo "窗口场景未运行：需要 FLOW_HOME 和 A3S_ORG_API_ORIGIN 指向这条故事的测试接口。" >&2
  exit 2
fi

echo "窗口场景未运行：前置条件已满足，但这条脚本不启动 ToWork，也不读取会员会话。" >&2
exit 2
