"""Reconcile the 153 source checkpoints with retained successful live observations."""
import hashlib
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
rows = []
for line in (CAMPAIGN / 'OUTCOME.md').read_text(encoding='utf-8').splitlines():
    match = re.fullmatch(r'\| (F\d{2}\.\d{2}) \| (.+) \|', line)
    if match:
        rows.append({'id': match[1], 'requirement': match[2], 'status': 'pending', 'observations': [], 'remaining': '尚未建立本检查点全部条件的双平台端到端证据。'})
assert len(rows) == 153 and len({row['id'] for row in rows}) == 153
assert len({row['id'].split('.')[0] for row in rows}) == 16
by_id = {row['id']: row for row in rows}
cache = {}

def evidence(platform, phase, case):
    relative = f'{platform}/{phase}/{case}.json'
    if relative not in cache:
        raw = (CAMPAIGN / relative).read_bytes()
        data = json.loads(raw)
        assert data['assertions_finished'] and not data['panicked'], relative
        assert data['config_unchanged'] and data['model'] == 'kimi/k3.1-pro', relative
        pair = next(item['detail'] for item in data['oracles'] if item['name'] == 'source-pair')
        cache[relative] = {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(), 'source_pair': pair, 'oracle_count': len(data['oracles']), 'named_oracles': [item['name'] for item in data['oracles'] if 'name' in item], 'metadata_record_count': sum('name' not in item for item in data['oracles'])}
    return cache[relative]

def put(point, status, cases, remaining='', phase='full-31-current-pair'):
    row = by_id[point]
    row['status'] = status
    row['remaining'] = remaining
    row['observations'] = [evidence(platform, phase, case) for platform in ('windows', 'linux') for case in cases]

put('F01.01', 'partial', ['missing-file', 'read'], '默认 Normal 和指定模型已验证；启动异常属于后续独立检查点。')
put('F01.02', 'partial', ['plan-read-write', 'plan-yolo-ceiling'], '显式 --plan 启动、真实 read、write 拒绝及 YOLO 上限已验证；有写副作用 exec 仍待独立 live 验证。')
put('F01.03', 'verified_for_listed_contract', ['read'])
put('F01.07', 'partial', ['interrupt', 'surface'], '正常退出和运行中取消已验证；启动中取消及异常退出后的终端模式仍待验证。')
put('F02.01', 'verified_for_listed_contract', ['trust-accept'], phase='trust-acceptance-short-fixture')
put('F02.02', 'verified_for_listed_contract', ['trust-reject', 'trust-cancel'], phase='trust-acceptance-short-fixture')
put('F02.03', 'verified_for_listed_contract', ['learned-approval'])
by_id['F02.03']['observations'].extend([
    evidence(platform, 'trust-acceptance-short-fixture', 'trust-accept')
    for platform in ('windows', 'linux')
])
put('F02.05', 'verified_for_listed_contract', ['workspace-acl-authority'], phase='workspace-acl-authority')
put('F02.06', 'verified_for_listed_contract', ['path-absolute-escape', 'path-relative-escape', 'path-link-escape', 'path-bounded-read'], phase='path-boundaries-diagnostic')
put('F02.04', 'verified_for_listed_contract', ['multiline-unicode'])
by_id['F02.04']['observations'].extend([
    evidence('windows', 'relative-cwd-qualified', 'relative-cwd'),
    evidence('linux', 'relative-cwd-final-binary', 'relative-cwd'),
])
put('F03.01', 'verified_for_listed_contract', ['multiline-unicode', 'long-unicode-paste'])
put('F03.02', 'verified_for_listed_contract', ['multiline-unicode'])
by_id['F03.02']['observations'].extend([
    evidence(platform, 'multiline-manual', case)
    for platform in ('windows', 'linux')
    for case in ('multiline-manual-enter', 'multiline-alt-backslash')
])
put('F03.03', 'verified_for_listed_contract', ['long-unicode-paste'])
put('F03.04', 'verified_for_listed_contract', ['draft-edit-controls'], phase='full-31-current-pair')
by_id['F03.04']['observations'].extend([
    evidence(platform, 'edit-combined', 'selection-edit-controls')
    for platform in ('windows', 'linux')
])
put('F03.05', 'verified_for_listed_contract', ['draft-edit-controls'], phase='full-31-current-pair')
by_id['F03.05']['observations'].extend([
    evidence(platform, 'edit-combined', 'multiline-edit-controls')
    for platform in ('windows', 'linux')
])
put('F03.06', 'verified_for_listed_contract', ['multiline-unicode'])
by_id['F03.06']['observations'].extend([
    evidence(platform, 'history-edit-resubmit', 'history-edit-resubmit')
    for platform in ('windows', 'linux')
])
for point in ('F03.07', 'F03.08'):
    row = by_id[point]
    row['status'] = 'verified_for_listed_contract'
    row['remaining'] = ''
    row['observations'] = [evidence(platform, 'full-31-current-pair', 'editor-' + case) for platform in ('windows', 'linux') for case in ('save', 'nonzero', 'invalid-utf8', 'missing-program')]
by_id['F03.08']['observations'].extend([
    evidence(platform, 'editor-cancel', 'editor-cancel')
    for platform in ('windows', 'linux')
])
put('F04.01', 'verified_for_listed_contract', ['preferences-restart', 'surface'])
by_id['F04.01']['observations'].extend([
    evidence(platform, 'session-isolation', 'session-isolation')
    for platform in ('windows', 'linux')
])
put('F04.02', 'verified_for_listed_contract', ['surface'])
by_id['F04.02']['observations'].extend([
    evidence(platform, 'home-return', 'home-return')
    for platform in ('windows', 'linux')
])
put('F04.03', 'verified_for_listed_contract', ['surface', 'read'])
by_id['F04.03']['observations'].extend([
    evidence(platform, 'session-info', 'session-info-contract')
    for platform in ('windows', 'linux')
])
put('F04.04', 'verified_for_listed_contract', ['resume-model-history'])
put('F04.05', 'verified_for_listed_contract', ['resume-model-history'])
by_id['F04.05']['observations'].extend([
    evidence(platform, 'continue-before', 'continue-latest-workspace')
    for platform in ('windows', 'linux')
])
put('F04.06', 'verified_for_listed_contract', ['session-picker-duplicate-title'], phase='picker-final')
by_id['F04.06']['observations'].extend([
    evidence(platform, 'picker-release-final', case)
    for platform in ('windows', 'linux')
    for case in ('session-picker-duplicate-title', 'session-picker-native-paste')
])
put('F04.08', 'partial', ['surface'], '确认删除及磁盘结果已验证；取消删除与其他会话不受影响仍待验证。')
for point in ('F05.01', 'F05.02', 'F05.03'):
    put(point, 'partial', ['surface'], '目录、选择恢复和 effort footer 已验证；真实换模推理与 effort transport/预算仍待逐项验证。')
for point in ('F05.04', 'F05.05', 'F05.06', 'F05.07', 'F05.08', 'F05.09', 'F05.10'):
    put(point, 'partial', ['config-crud-restart'], 'CRUD、取消、reload、保存、重启及后续 Kimi 请求已验证；完整字段边界和实际请求参数仍待逐项验证。')
put('F05.11', 'partial', ['surface'], '实际轮次后上下文面板已验证；used/total/free 数值仍待独立核对。')
put('F05.12', 'partial', ['compact-model-history'], '有/无 focus 的真实压缩及后续无工具回忆已验证；失败恢复仍待验证。')
put('F06.01', 'partial', ['read', 'coding'], '真实流式请求与最终响应已验证；完整 delta/最终正文逐项对应仍待验证。')
put('F06.02', 'partial', ['write-verify', 'expand'], '真实工具卡与结果已验证；全部调用的 UI 卡片计数和更新归属仍待验证。')
put('F06.07', 'partial', ['interrupt'], '运行中 shell 取消及八秒内无晚到写入已验证；单独取消模型流仍待验证。')
put('F06.08', 'partial', ['yolo-explicit-deny', 'missing-file'], '拒绝后的同会话继续可用已验证；provider/ACP 故障后的重连和幂等恢复仍待验证。')
put('F07.01', 'partial', ['read', 'missing-file', 'plan-read-write'], '真实读、缺失和 EOF 字节已验证；范围/分页/越界的完整 live 组合仍待验证。')
put('F07.04', 'partial', ['write-verify', 'plan-read-write'], '实际写入与无尾换行精确字节已验证；Unicode、CRLF、覆盖及失败原子性仍待完整验证。')
put('F07.05', 'partial', ['coding'], '真实多轮修改与独立 checker 已验证；多重匹配和未匹配仍待验证。')
put('F07.07', 'partial', ['write-verify', 'coding', 'learned-approval'], '原生 shell 成功与真实输出已验证；非零退出、stderr 和全部引用形式仍待验证。')
put('F07.08', 'partial', ['interrupt'], '运行中取消已验证；超时和输出截断仍待独立验证。')
put('F07.18', 'partial', ['expand', 'write-verify'], '真实工具输出与展开尾部已验证；全部 diff/raw/card 形式仍待验证。')
put('F07.19', 'verified_for_listed_contract', ['coding'])
put('F08.01', 'partial', ['allow-once', 'permission-reject', 'write-verify'], 'Normal 与 YOLO 已验证；Auto 是明确禁用入口，不作为可用功能计数。')
put('F08.02', 'partial', ['allow-once'], '实际 Allow once 已验证；相同工具下一次未经授权调用仍待单独验证。')
put('F08.03', 'verified_for_listed_contract', ['learned-approval'])
put('F08.04', 'verified_for_listed_contract', ['learned-approval'])
put('F08.05', 'partial', ['permission-reject', 'permission-cancel'], '两种 UI 操作无写入已验证；真实拒绝与取消结果的独立归属仍待验证。')
put('F08.06', 'verified_for_listed_contract', ['yolo-explicit-deny', 'write-verify'])
put('F08.07', 'partial', ['yolo-explicit-deny'], '实际 CLI deny 跨 YOLO 仍有效；全部重开/换模组合仍待 live 验证。')
put('F08.08', 'partial', ['plan-yolo-ceiling'], 'Plan+YOLO 的写入拒绝已验证；学习规则和有写副作用 exec 组合仍待验证。')
put('F08.11', 'partial', ['learned-approval'], '实际 ACL 学习和路径规则已验证；全部参数/路径形式仍待 live 验证。')
put('F09.01', 'partial', ['plan-read-write', 'plan-yolo-ceiling'], 'Plan 的 read 和 write 上限已验证；search 与有写副作用 exec 仍待验证。')
put('F09.06', 'partial', ['plan-read-write', 'plan-yolo-ceiling'], '真实模式切换和权限上限已验证；计划审批的完整入口组合仍待验证。')
put('F10.01', 'verified_for_listed_contract', ['goal'])
for point in ('F10.02', 'F10.04', 'F10.05'):
    put(point, 'partial', ['goal', 'surface'], '目标控制 durable facts 已验证；停止后续工作、预算累计、跨会话和完整状态仍待验证。')
for point in ('F11.02', 'F11.03', 'F11.04', 'F11.05'):
    put(point, 'partial', ['form-submit', 'form-cancel'], '真实原生表单的代表性字段、校验、提交和取消已验证；全部组件、约束和焦点组合仍待验证。')
for point in ('F12.01', 'F12.02'):
    put(point, 'verified_for_listed_contract', ['reading-navigation'], phase='full-31-current-pair')
put('F12.03', 'partial', ['reading-navigation'], '实际 turn 选择和取消恢复已验证；当前 jump picker 无筛选输入，不能宣称已验证筛选。', phase='full-31-current-pair')
put('F12.04', 'partial', ['reading-navigation'], '实际顶部/底部已验证；行、半页、整页和轮次/响应组合仍待验证。', phase='full-31-current-pair')
put('F12.05', 'partial', ['expand'], '真实工具展开尾部已验证；单块/全部/思考的独立折叠控制仍待验证。')
for point in ('F13.01', 'F13.03', 'F13.05', 'F13.07', 'F13.09', 'F13.10'):
    put(point, 'partial', ['preferences-restart', 'surface'], '对应代表性设置、模式或入口有实际证据；本检查点的全部控件/渲染/焦点组合仍待验证。')
put('F13.06', 'partial', ['reading-navigation'], '真实 Vim 顶部/底部导航已验证；选择、折叠、搜索及编辑组合仍待验证。', phase='full-31-current-pair')
put('F13.08', 'partial', ['reading-navigation'], '三种 PTY 尺寸下正文位置可达已验证；所有 modal/必填/确认控件仍待验证。', phase='full-31-current-pair')
for point in ('F14.01', 'F14.02', 'F14.03', 'F14.07'):
    put(point, 'partial', ['copy-export-contract'], '指定响应精确文件字节、导出顺序和部分失败已验证；系统 clipboard、恢复及全部编码/边界仍待验证。')
put('F16.01', 'verified_for_listed_contract', ['backend-refusal'])
for point in ('F16.03', 'F16.04'):
    put(point, 'partial', ['paste-help-capability', 'history-help-capability', 'sessions-help-capability'], '三个实际帮助详情已修复并验证；全部快捷键/面板/CLI 的一致性仍待复核。', phase='full-31-current-pair')
put('F16.05', 'partial', ['surface'], 'doctor 实际入口已运行；每项诊断和缺依赖故障仍待验证。')
put('F16.09', 'partial', ['read', 'copy-export-contract'], '测试隔离和原 ACL 字节不变已逐例核对；新增全部 evidence 的最终凭据精确扫描仍待执行。')

put('F03.09', 'verified_for_listed_contract', ['file-ref-selection', 'file-ref-missing', 'file-ref-outside'], phase='file-references')
put('F03.10', 'explicitly_unavailable', ['direct-shell-capability'], 'Code 后端明确拒绝直接 shell 协议，不把拒绝计为可用执行功能。', phase='full-31-current-pair')
for point in ('F03.11', 'F06.05'):
    put(point, 'verified_for_listed_contract', ['queue-edit-delete-drain'], phase='full-31-current-pair')
    by_id[point]['observations'].extend([
        evidence(platform, 'queue-current', case)
        for platform in ('windows', 'linux')
        for case in ('queue-edit-delete-drain', 'send-now-cancel-ownership', 'send-now-model-stream')
    ])
put('F06.04', 'verified_for_listed_contract', ['queue-edit-delete-drain'], phase='full-31-current-pair')

def counterexample(platform, phase, case):
    relative = f'{platform}/{phase}/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    data = json.loads(raw)
    assert data['panicked'] and not data['assertions_finished']
    assert data['config_unchanged'] and data['model'] == 'kimi/k3.1-pro'
    pair = next(item['detail'] for item in data['oracles'] if item.get('name') == 'source-pair')
    return {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(), 'source_pair': pair, 'result': 'reproduced_failure'}

by_id['F03.09']['counterexamples'] = [
    counterexample(platform, phase, case)
    for platform in ('windows', 'linux')
    for phase, case in [
        ('file-references-before-fix', 'file-ref-missing'),
        ('file-references-boundary-before-fix', 'file-ref-outside'),
    ]
]
by_id['F04.01']['counterexamples'] = [
    counterexample(platform, 'session-isolation-before-fix', 'session-isolation')
    for platform in ('windows', 'linux')
]
by_id['F04.03']['counterexamples'] = [
    counterexample(platform, 'session-info-before-mode', 'session-info-contract')
    for platform in ('windows', 'linux')
]
by_id['F04.06']['counterexamples'] = [
    counterexample(platform, 'picker-before', 'session-picker-duplicate-title')
    for platform in ('windows', 'linux')
]
by_id['F04.06']['counterexamples'].append(
    counterexample('windows', 'picker-native-paste-before', 'session-picker-native-paste')
)

for point, case, windows_phase, linux_phase in [
    ('F06.06', 'send-now-cancel-ownership', 'tool-contracts-send-now-isolated', 'tool-contracts-isolated-send_now_cancels_running_tool_and_runs_new_prompt_once'),
    ('F07.07', 'shell-nonzero-contract', 'tool-contracts-shell-isolated', 'tool-contracts-isolated-native_shell_preserves_nonzero_stdout_stderr_cwd_and_recovers'),
]:
    by_id[point]['counterexamples'] = [counterexample(platform, phase, case) for platform, phase in [('windows', windows_phase), ('linux', linux_phase)]]
by_id['F06.06']['status'] = 'partial'
by_id['F06.06']['remaining'] = '原生工具取消、后续正文逐字保持及未验证副作用的明确拒绝已在两平台验证；纯模型取消仍错误追加完成标记，修复后的同源码完整 live 验证仍在进行。'
for point in ('F03.11', 'F06.06', 'F06.07'):
    by_id[point]['observations'].extend([
        evidence('windows', 'owed-guidance-extended-tools', 'send-now-cancel-ownership'),
        evidence('linux', 'owed-guidance-cancel-early', 'send-now-cancel-ownership'),
    ])
for point in ('F06.06', 'F06.07'):
    by_id[point].setdefault('counterexamples', []).extend([
        counterexample(platform, 'owed-model-cancel-isolated', 'send-now-model-stream')
        for platform in ('windows', 'linux')
    ])
for point in ('F03.11', 'F06.06', 'F06.07'):
    by_id[point]['observations'].extend([
        evidence(platform, 'completion-facts-cancel', case)
        for platform in ('windows', 'linux')
        for case in ('send-now-cancel-ownership', 'send-now-model-stream')
    ])
by_id['F03.11']['remaining'] = ''
by_id['F06.06']['remaining'] = '真实工具和模型流取消后，后续消息字节与归属均已双平台验证；历史完成标记错误保留反例，最终同源码完整批次仍待完成。'
by_id['F06.07']['remaining'] = '真实模型部分响应和运行中工具均已取消，后续消息完成或明确拒绝，无晚到写入；最终同源码完整批次及全部子任务组合仍待完成。'
by_id['F07.07']['observations'].append(evidence('windows', 'owed-guidance-extended-tools', 'shell-nonzero-contract'))
by_id['F07.07']['observations'].append(evidence('linux', 'owed-guidance-native-shell', 'shell-nonzero-contract'))
by_id['F07.07']['counterexamples'].append(counterexample('linux', 'owed-guidance-native-shell-isolated', 'shell-nonzero-contract'))
by_id['F07.07']['remaining'] = '已有真实失败脚本的 exit 7、双输出流、原生 cwd、含空格脚本名和继续请求证据；历史模型回答遗漏仍保留反例，最终同源码双平台批次及全部引用形式仍待验证。'
for point, remaining in [
    ('F07.02', '目录优先排序与 limit=2 的真实结果已验证；全部路径、工作区边界和目录条目类型仍待覆盖。'),
    ('F07.03', '真实未知内容、限定目录、上下文、分页 cursor、无匹配及不支持组合的拒绝已验证；位置/全部范围边界仍待覆盖。'),
]:
    by_id[point]['status'] = 'partial'
    by_id[point]['remaining'] = remaining
    by_id[point]['observations'] = [
        evidence('windows', 'owed-guidance-extended-tools', 'ls-search-contracts'),
        evidence('linux', 'owed-guidance-ls-search', 'ls-search-contracts'),
    ]
by_id['F06.06']['status'] = 'verified_for_listed_contract'
by_id['F06.06']['remaining'] = ''
by_id['F06.06']['observations'] = [
    evidence(platform, 'full-35-qualification-final', case)
    for platform in ('windows', 'linux')
    for case in ('send-now-cancel-ownership', 'send-now-model-stream')
]
for point in ('F03.04', 'F03.05'):
    by_id[point].setdefault('counterexamples', []).append(counterexample('linux', 'full-35-qualification-final', 'draft-edit-controls'))
    by_id[point]['observations'].extend([
        evidence('windows', 'full-35-qualification-final', 'draft-edit-controls'),
        evidence('linux', 'draft-edit-32b8-isolated', 'draft-edit-controls'),
    ])
    by_id[point]['remaining'] += ' 最新 Linux 完整批次在 Ctrl+W 后 200 ms 的屏幕断言失败，单独复跑通过；输入与显示同步的稳定性仍待验证。'
for point in ('F16.03', 'F16.04'):
    by_id[point]['observations'].extend([
        evidence(platform, 'full-35-qualification-final', 'send-now-help-capability')
        for platform in ('windows', 'linux')
    ])
for platform, case, points in [
    ('windows', 'ls-search-contracts', ('F07.02', 'F07.03')),
    ('linux', 'yolo-explicit-deny', ('F08.01', 'F08.10')),
    ('linux', 'shell-nonzero-contract', ('F07.07',)),
]:
    for point in points:
        by_id[point].setdefault('counterexamples', []).append(counterexample(platform, 'full-35-ui-tail-readiness', case))
by_id['F07.07']['counterexamples'].append(counterexample('linux', 'shell-43ce-isolated', 'shell-nonzero-contract'))
by_id['F07.07']['observations'].extend([
    evidence(platform, 'full-35-batchfix-continuation', 'shell-nonzero-contract')
    for platform in ('windows', 'linux')
])
by_id['F07.07']['remaining'] = '真实 exit 7、双输出、cwd 和最终模型答复已在 a891 双端完整批次中保持一致；历史遗漏反例保留，其他引用、截断和超时形式仍待验证。'
for point in ('F07.02', 'F07.03'):
    by_id[point]['observations'].append(evidence('windows', 'ls-search-43ce-isolated', 'ls-search-contracts'))
    by_id[point]['observations'].extend([
        evidence(platform, 'full-35-batchfix-continuation', 'ls-search-contracts')
        for platform in ('windows', 'linux')
    ])
    by_id[point]['remaining'] += ' a891 双端完整批次通过；历史偶发反例保留，全部路径和范围边界仍待覆盖。'
for point in ('F08.01', 'F08.10'):
    by_id[point]['observations'].append(evidence('linux', 'deny-43ce-isolated', 'yolo-explicit-deny'))
    by_id[point]['observations'].extend([
        evidence(platform, 'full-35-batchfix-continuation', 'yolo-explicit-deny')
        for platform in ('windows', 'linux')
    ])
    by_id[point]['remaining'] += ' a891 双端完整批次中的真实拒绝与后续读取通过；全部 sandbox 和模式组合仍待验证。'
for point in ('F03.04', 'F03.05'):
    by_id[point]['observations'].extend([
        evidence(platform, phase, 'draft-edit-controls')
        for platform in ('windows', 'linux')
        for phase in ('full-35-ui-tail-readiness', 'startup-errors-pilot')
    ])
    by_id[point]['remaining'] = ''


def native_evidence(platform, phase, cases):
    relative = f'{platform}/{phase}/report.json'
    raw = (CAMPAIGN / relative).read_bytes()
    data = json.loads(raw)
    assert data['all_passed'] and data['config_unchanged']
    assert data['source_pair'].split(':')[1] == '189982670423c6c055335d5e701b491d295583e5'
    found = {item['case']: item for item in data['cases']}
    for case in cases:
        item = found[case]
        assert item['result'] == 'pass' and not item['timed_out']
        if case == 'scheduler-confirmation-cancel':
            assert item['confirmation_cancelled_before_execution'] and item['no_file_effect']
            assert item['model_requested_tools'] == ['write'] and item['confirmation_answers'] == [False]
    return {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
            'source_pair': data['source_pair'], 'script_sha256': data['script_sha256'], 'native_cases': cases}


for point, cases in [
    ('F01.04', ['missing-companion', 'mismatched-companion', 'unexecutable-companion']),
    ('F01.05', ['missing-acl', 'invalid-acl', 'unknown-model']),
]:
    by_id[point]['status'] = 'verified_for_listed_contract'
    by_id[point]['remaining'] = ''
    by_id[point]['observations'] = [native_evidence(platform, 'native-cli-startup-errors', cases) for platform in ('windows', 'linux')]
by_id['F01.06']['status'] = 'verified_for_listed_contract'
by_id['F01.06']['remaining'] = ''
by_id['F01.06']['observations'] = [
    item for platform in ('windows', 'linux') for item in [
        evidence(platform, 'full-35-ui-tail-readiness', 'permission-cancel'),
        native_evidence(platform, 'native-cli-startup-errors', ['headless-json', 'headless-relative-cwd']),
        native_evidence(platform, 'native-cli-confirmation-cancel', ['scheduler-confirmation-cancel']),
    ]
]
by_id['F01.06']['qualification_note'] = '调度器在 confirmation.answered=false 后结束，尚未进入工具准备/执行；原工具回执预期不适用的测试失败和原脚本仍保留，不作为功能失败或成功证据。'
by_id['F02.04']['observations'].extend([
    native_evidence(platform, 'native-cli-startup-errors', ['headless-relative-cwd']) for platform in ('windows', 'linux')
])
by_id['F02.04']['remaining'] = '两平台中文/空格相对 --cwd 的 headless 真实模型请求已修复验证，正确目录产生精确 user.message；交互 TUI 的相对启动及其他平台路径形式仍待验证。'


def native_counterexample(platform, phase, case):
    relative = f'{platform}/{phase}/report.json'
    raw = (CAMPAIGN / relative).read_bytes()
    data = json.loads(raw)
    assert data['config_unchanged'] and not data['all_passed']
    item = next(item for item in data['cases'] if item['case'] == case)
    assert item['result'] == 'fail'
    return {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
            'source_pair': data['source_pair'], 'script_sha256': data['script_sha256'], 'native_case': case}


for platform in ('windows', 'linux'):
    by_id['F01.04'].setdefault('counterexamples', []).append(native_counterexample(platform, 'native-cli-unexecutable-initial', 'unexecutable-companion'))
    phase = 'native-cli-invalid-acl' if platform == 'windows' else 'native-cli-invalid-acl-initial'
    by_id['F01.05'].setdefault('counterexamples', []).append(native_counterexample(platform, phase, 'invalid-acl'))
    by_id['F02.04'].setdefault('counterexamples', []).append(native_counterexample(platform, 'native-cli-relative-cwd', 'headless-relative-cwd'))
    by_id['F01.06'].setdefault('excluded_evaluations', []).append(native_counterexample(platform, 'native-cli-non-tty-permission', 'non-tty-permission'))
put('F04.07', 'verified_for_listed_contract', ['background-session-read-only-owner'], phase='continuation-20261030-owner')
by_id['F16.01']['status'] = 'partial'
by_id['F16.01']['remaining'] = '当前 e1a37d3 Windows 57 项复跑在 /workflow 等待拒绝提示时失败；原始记录显示命令仍在输入框。输入就绪判据或提交链路需排查，42 项完整双平台复验仍未成立。历史通过记录保留。'
relative = 'windows/continuation-20261030-all-57/backend-refusal.json'
raw = (CAMPAIGN / relative).read_bytes()
failed = json.loads(raw)
assert failed['panicked'] and not failed['assertions_finished'] and failed['config_unchanged']
by_id['F16.01'].setdefault('counterexamples', []).append({
    'platform': 'windows', 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
    'source_pair': next(item['detail'] for item in failed['oracles'] if item.get('name') == 'source-pair'),
    'classification': 'evaluation failure; input-readiness/submission attribution pending',
})
for platform in ('windows', 'linux'):
    by_id['F04.03']['observations'].append(evidence(platform, 'continuation-20261030-session-info-proxy', 'session-info-contract'))
put('F16.01', 'verified_for_listed_contract', ['backend-refusal'], phase='continuation-20261030-refusal-composer')
by_id['F16.01']['remaining'] = '当前固定 e1a37d3 产品上的独立补跑已在双平台逐项验证 42 个禁用命令及一个未知命令；驱动仅改为等待实际输入框中的精确草稿，提示、超时、断言未变。原始 57 项批次失败及未改动驱动的独立失败仍保留，不声称统一完整批次通过。'
by_id['F16.01']['counterexamples'][-1]['classification'] = 'harness readiness: prior /workflows refusal matched /workflow before the next draft reached the composer; unchanged repro failed and exact-draft follow-up passed on both native platforms'
for point in ('F01.02', 'F09.01'):
    put(point, 'verified_for_listed_contract', ['plan-read-write', 'plan-yolo-ceiling'], phase='continuation-20261030-all-57')
    by_id[point]['observations'].extend([
        evidence(platform, 'continuation-20261030-plan-search-exec', 'plan-search-exec-deny')
        for platform in ('windows', 'linux')
    ])
put('F11.07', 'verified_for_listed_contract', ['form-schema-refusals'], phase='questions-20261030-initial')
by_id['F11.01']['status'] = 'partial'
by_id['F11.01']['remaining'] = 'Windows 真实选项、禁用自由输入及未见过的 Unicode 自由输入已按原 question id 验证。取消后当前运行仍等待答案的问题已在双平台复现并修复，新的 ACP cancelled 协议验收尚在运行；Linux 原自由输入评估另有模型未调用工具的失败，提示与断言保留。'
by_id['F11.01']['observations'] = [evidence('windows', 'questions-20261030-initial', 'question-choice-free-text')]
for platform, case, classification in [
    ('windows', 'question-cancel-recovery', 'cancel dialog closes but native turn remains active beyond 90 seconds; the historical grader also incorrectly demands a success completion fact for cancellation'),
    ('linux', 'question-cancel-recovery', 'cancel dialog closes but native turn remains active beyond 90 seconds; the historical grader also incorrectly demands a success completion fact for cancellation'),
    ('linux', 'question-choice-free-text', 'provider returned a plain text question instead of calling ask_user for the second fixed prompt'),
]:
    relative = f'{platform}/questions-20261030-initial/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    by_id['F11.01'].setdefault('counterexamples', []).append({
        'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': next(item['detail'] for item in report['oracles'] if item.get('name') == 'source-pair'),
        'classification': classification,
    })
forms_layout_phase = 'forms-20261030-layout-qualified-pair/reports'
forms_error_phase = 'forms-20261030-error-qualified-pair/reports'
put('F11.01', 'verified_for_listed_contract',
    ['question-choice-free-text', 'question-cancel-protocol-recovery'], phase=forms_layout_phase)
by_id['F11.01']['qualification_note'] = '真实选择、禁用自由输入、Unicode 自由输入按 question id 核对。取消以 ACP cancelled、原会话空闲、无合成默认答案及后续真实请求成功判定；不要求取消产生成功完成事实。历史失败评估保留。'
put('F11.02', 'verified_for_listed_contract', ['form-catalog-native-profile'], phase=forms_layout_phase)
by_id['F11.02']['qualification_note'] = '固定文档的 13 种不同类型与共享组件目录完全相等；验证原生文本映射、数值单位及八个可编辑字段。此合同不声称每种组件的全部属性组合均已验证。'
by_id['F11.03']['observations'].extend([
    evidence(platform, forms_error_phase, 'form-constraint-edits')
    for platform in ('windows', 'linux')
])
by_id['F11.03']['remaining'] = '双端真实 required、数字类型、min/max、受限枚举及 required boolean 拒绝和修改后准确提交均已验证。Slider/Input 的 step 行为及其他属性组合尚未建立端到端证据；当前共享解析器校验 step 参数但不传入字段 schema，不能据此声称全部约束通过。'
put('F11.04', 'verified_for_listed_contract', ['form-catalog-native-profile'], phase=forms_layout_phase)
by_id['F11.04']['observations'].extend([
    evidence(platform, forms_error_phase, case)
    for platform in ('windows', 'linux')
    for case in ('form-constraint-edits', 'form-long-small-window')
])
put('F11.05', 'verified_for_listed_contract', ['form-cancel-continue'], phase=forms_layout_phase)
by_id['F11.05']['observations'].extend([
    evidence(platform, forms_error_phase, 'form-long-small-window')
    for platform in ('windows', 'linux')
])
put('F11.08', 'verified_for_listed_contract', ['form-long-small-window'], phase=forms_error_phase)
for platform, phase, case, point, classification in [
    (platform, phase, case, point, classification)
    for platform in ('windows', 'linux')
    for phase, case, point, classification in [
        ('questions-20261030-qualified', 'form-long-small-window', 'F11.08',
         'product layout: five reserved scrollback rows hide the required fields and actions in a 14x64 terminal'),
        ('forms-20261030-layout-qualified-pair/reports', 'form-long-small-window', 'F11.08',
         'product layout: fields and actions are reachable, but the required validation error is outside the viewport while actions have focus'),
        ('questions-20261030-qualified', 'form-all-components', 'F11.02',
         'excluded evaluation: integer metric formatting and simultaneous visibility of every field contradict the native shared-parser display and scrollable viewport'),
    ]
]:
    relative = f'{platform}/{phase}/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    by_id[point].setdefault('counterexamples', []).append({
        'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': next(item['detail'] for item in report['oracles'] if item.get('name') == 'source-pair'),
        'classification': classification,
    })
relative = 'linux/forms-20261030-layout-qualified-pair/reports/form-constraint-edits.json'
raw = (CAMPAIGN / relative).read_bytes()
report = json.loads(raw)
assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
by_id['F11.03'].setdefault('counterexamples', []).append({
    'platform': 'linux', 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
    'source_pair': next(item['detail'] for item in report['oracles'] if item.get('name') == 'source-pair'),
    'classification': 'harness screen parsing: the exact required error is visible inside the Linux heavy border; follow-up accepts both native decorative borders and retains the exact error-text assertion',
})
owner_step_phase = 'forms-20261030-owner-step-initial'
put('F11.06', 'verified_for_listed_contract', ['form-session-owner'], phase=owner_step_phase + '/reports')
by_id['F11.06']['qualification_note'] = '真实 A/B 会话切换、B 的草稿与后续模型请求、A 的两次 Enter 仅产生一个精确表单回执均在双平台通过。原生处理器测试额外保留旧 A 响应通道，使其在新 A 和 B 均待答时迟到，再重复旧 resolved 通知；新通道不被回答或关闭，两个新答案及草稿各归原主。'
for platform in ('windows', 'linux'):
    relative = f'{platform}/{owner_step_phase}/owner-library.log'
    raw = (CAMPAIGN / relative).read_bytes()
    text = raw.decode('utf-8-sig')
    assert 'late_and_duplicate_form_answers_keep_their_request_and_session_owner ... ok' in text
    assert 'test result: ok. 1 passed; 0 failed; 0 ignored;' in text
    manifest = json.loads((CAMPAIGN / platform / owner_step_phase / 'test-binary.json').read_text())
    assert manifest['source_pair'].split(':')[1] == 'b44752d77fa649bf767498820c1fdb123aaa13ac'
    by_id['F11.06']['observations'].append({
        'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': manifest['source_pair'], 'native_library_test': 'app::acp_handler::form::tests::late_and_duplicate_form_answers_keep_their_request_and_session_owner',
    })
for platform, phase, key, classification in [
    ('windows', 'forms-20261030-step-exact-reprobe', 'counterexamples',
     'product defect: native Slider step=2 accepts level=3 and returns it in a successful real tool receipt'),
    ('linux', owner_step_phase, 'counterexamples',
     'product defect: native Slider step=2 accepts level=3 and returns it in a successful real tool receipt'),
    ('windows', owner_step_phase, 'excluded_evaluations',
     'model transcription error: the fixed source document was copied into malformed JSON; the real tool rejects it before any form opens'),
]:
    relative = f'{platform}/{phase}/reports/form-step-constraints.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    by_id['F11.03'].setdefault(key, []).append({
        'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
        'classification': classification,
    })
by_id['F11.03']['remaining'] = 'required、数字类型、min/max、受限枚举及 required boolean 的拒绝、修改和精确提交已双端通过。新增实测确认产品缺陷：Slider step=2 仍接受并在成功回执返回 3，Windows 原评估的模型转写错误另行保留。步长修复、Input 小数 step 及其他属性组合仍待验收。'
tool_initial = 'tool-wave-20261030-native/reports'
tool_qualified = 'tool-wave-20261030-qualified/reports'
put('F07.15', 'verified_for_listed_contract', ['tool-wave-batch'], phase=tool_initial)
put('F07.16', 'partial', ['tool-wave-program'], phase=tool_initial,
    remaining='真实嵌套 read、完整回执、allowed_tools 越权拒绝和 Plan 的原始 denied 回执均已双平台验证；注册 template catalog 与其他限制边界尚未完整验收。')
by_id['F07.16']['observations'].extend([
    evidence(platform, tool_qualified, 'tool-wave-program-plan-policy') for platform in ('windows', 'linux')
])
for point in ('F15.01', 'F15.03'):
    put(point, 'verified_for_listed_contract', ['tool-wave-task-children-traced'], phase=tool_qualified)
put('F15.02', 'partial', ['tool-wave-task-children-traced'], phase=tool_qualified,
    remaining='两个真实 child 的独立输出、source anchors、开始/进度/结束通知及父级等待均已验证；子任务面板中的逐项选择、文本和工具视图仍待原生交互验收。')
put('F15.07', 'partial', ['tool-wave-task-children-traced'], phase=tool_qualified,
    remaining='真实 agent child 与 background shell 的身份、调用族、输出和结束回执已分开留证；background 运行以异常删除投影触发完成门禁，取消后的完整观察窗口尚未到达，不能声明两族取消均通过。')
by_id['F07.06']['status'] = 'partial'
by_id['F07.06']['observations'] = [evidence('linux', tool_qualified, 'tool-wave-patch-verified')]
by_id['F07.06']['remaining'] = 'WSL 单文件、多文件全成功、第二文件冲突导致全事务不变、真实命令验证和精确文件字节均通过；Windows 已执行单文件 patch，但 PowerShell 验证命令被冻结策略拒绝，后续多文件/冲突分支未到达。'
by_id['F07.10']['status'] = 'partial'
by_id['F07.10']['remaining'] = '双端真实 start/tail/stop/tail 和独立 shell 检查均执行；stop 后再次 tail 明确找不到该 Run 的任务。随后出现同一 23 路径删除投影，完成门禁失败，61 秒晚到写入观察未到达；根因与完整取消合同仍未验收。'
by_id['F07.11']['status'] = 'partial'
by_id['F07.11']['remaining'] = 'Windows 实际 git CLI 初始化的 fixture 被产品误判不是仓库（规范化路径带扩展前缀）；WSL summary、精确 diff 和分支创建已执行，但只请求 branch 创建后当前分支也切换，未满足不 checkout 的断言。坏 ref 分支未到达，两个问题均保留。'
by_id['F07.14']['status'] = 'verified_for_listed_contract'
by_id['F07.14']['remaining'] = ''
by_id['F07.14']['observations'] = [
    evidence('windows', 'tool-wave-20261030-checklist-exit/reports', 'tool-wave-checklist-exit'),
    evidence('linux', tool_qualified, 'tool-wave-checklist-traced'),
]
by_id['F07.14']['qualification_note'] = '同一 c860 产品源码：真实 checklist 从三项替换为两项，顺序、完成数量、取消元数据和原生显示对应。Windows 的独立补测在全部相同业务断言之后使用 overlay Space 返回 prompt 再退出；原 trace 版本的退出失败保留。'

for platform, phase, case, points, key, classification in [
    (platform, phase, case, points, key, classification)
    for platform in ('windows', 'linux')
    for phase, case, points, key, classification in [
        (tool_initial, 'tool-wave-patch', ('F07.06',), 'excluded_evaluations',
         'evaluation prompt excludes bash; successful patch plus read cannot bind the required command verification, so the unchanged completion gate refuses the turn'),
        (tool_initial, 'tool-wave-program-plan', ('F07.16',), 'excluded_evaluations',
         'harness parses the real policy denied receipt as JSON; denial has complete empty effects and no nested write'),
        (tool_initial, 'tool-wave-checklist', ('F07.14',), 'excluded_evaluations',
         'harness does not enable its required protocol trace; Windows also types the second request while the checklist owns focus'),
        (tool_initial, 'tool-wave-task-children', ('F15.01', 'F15.03'), 'excluded_evaluations',
         'real child result assertions pass, then the unconfigured optional ACP trace file is opened and is absent'),
        (tool_initial, 'tool-wave-background', ('F07.10', 'F15.07'), 'counterexamples',
         'actual run fails completion gate with the same 23-path deletion effect on both platforms; cancellation observation after parent completion is not reached; attribution remains open'),
        (tool_qualified, 'tool-wave-git-summary', ('F07.11',), 'counterexamples',
         'Windows rejects an existing fixture repo with an extended canonical path; Linux branch creation also changes the current branch despite the no-checkout instruction'),
    ]
]:
    relative = f'{platform}/{phase}/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    item = {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
            'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
            'classification': classification}
    for point in points:
        by_id[point].setdefault(key, []).append(item)
for platform, phase, case, point, key, classification in [
    ('windows', tool_qualified, 'tool-wave-patch-verified', 'F07.06', 'counterexamples',
     'real patch succeeds; the exact PowerShell verification command receives a policy denied receipt, so there is no bound Passed verification'),
    ('windows', tool_initial, 'tool-wave-git', 'F07.11', 'counterexamples',
     'real git tool reports Not a git repository for the actual initialized Windows fixture'),
    ('linux', tool_initial, 'tool-wave-git', 'F07.11', 'excluded_evaluations',
     'initial grader requires per-file names from git status although its published response is branch/commit/dirty-count summary; the later diff assertion remains exact'),
    ('windows', tool_qualified, 'tool-wave-checklist-traced', 'F07.14', 'excluded_evaluations',
     'all checklist business assertions and checkpoint succeed; teardown types /quit in the still-focused checklist filter'),
]:
    relative = f'{platform}/{phase}/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    by_id[point].setdefault(key, []).append({
        'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
        'classification': classification,
    })

for point in ('F07.06', 'F07.14', 'F07.15', 'F07.16', 'F15.01', 'F15.02', 'F15.03', 'F15.07'):
    linked = set()
    for observation in list(by_id[point]['observations']):
        platform = observation['platform']
        phase = observation['file'].split('/')[1]
        if (platform, phase) in linked:
            continue
        linked.add((platform, phase))
        for name in ('test-binary.json', 'harness-source.json'):
            relative = f'{platform}/{phase}/{name}'
            path = CAMPAIGN / relative
            if not path.exists():
                continue
            raw = path.read_bytes()
            data = json.loads(raw)
            pair = data.get('source_pair', data.get('runtime_pair'))
            assert pair == observation['source_pair']
            by_id[point]['observations'].append({
                'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
                'source_pair': pair, 'native_test_provenance': True,
            })
coverage_phase = 'coverage-20261030-portable/reports'
coverage_ui_phase = 'coverage-20261030-ui-qualified/reports'
for platform in ('windows', 'linux'):
    for case in ('coverage-batch-staged', 'coverage-batch-missing-reference'):
        by_id['F07.15']['observations'].append(evidence(platform, coverage_phase, case))
    for case in ('coverage-program-repo-map', 'coverage-program-typed-error',
                 'coverage-program-call-limit', 'coverage-program-timeout', 'coverage-program-output-limit'):
        by_id['F07.16']['observations'].append(evidence(platform, coverage_phase, case))
by_id['F07.16']['observations'].append(evidence('linux', coverage_phase, 'coverage-program-code-search'))
by_id['F07.16']['remaining'] = '两个默认注册模板已在 WSL 实际执行；Windows repo_map 的七个子步骤、双端参数失败归属、调用次数、100ms VM 超时后恢复及 256 字节 UTF-8 输出上限均通过。Windows code_search 在配置的 10000ms 预算下报超时，其唯一嵌套 search 成功但耗时 30048ms；保留该反例，未改变原预算复判通过。'
put('F07.13', 'partial', ['coverage-web-search-wiki'], phase=coverage_phase,
    remaining='显式匿名 wiki 引擎的真实 JSON 结果、标题和 HTTP 源链接已双平台通过；其他引擎、无服务条件和引用交互尚未完整验收。')
put('F12.06', 'verified_for_listed_contract', ['coverage-markdown-last-match'], phase=coverage_ui_phase)
by_id['F12.06']['observations'].append(evidence('windows', coverage_phase, 'coverage-markdown-raw'))
by_id['F12.06']['qualification_note'] = '真实 assistant Markdown 在格式化/Raw 间往返，关闭后 /copy 的 UTF-8 字节等于规范事实中的原回答，局部导航不产生后端事实。补测选择最后一个实际搜索匹配，兼容 Thinking 中多出的匹配；原固定 1/2 断言失败保留。'
by_id['F12.07']['status'] = 'partial'
by_id['F12.07']['remaining'] = '双平台实际 read 回执均包含 120 行及尾行 VIEW-ROW-119，但 block viewer 到 End 仅显示至约第 76 行且正文在行中截断；ACP 文本预览固定取 4000 字符。完整内容合同失败，后续 Home/关闭重开焦点断言未到达，产品未修复。'
put('F13.02', 'partial', ['coverage-theme-background'], phase=coverage_ui_phase,
    remaining='真实 Theme enum 的 Day 即时预览、不提前持久化、Esc 恢复、Enter 提交、reset No/Yes 均双端通过，并比较实际终端背景色；设置校验错误及其他 enum 类型组合仍待验收。')
by_id['F13.02']['observations'].append(evidence('linux', coverage_phase, 'coverage-theme-enum'))
for platform in ('windows', 'linux'):
    by_id['F13.03']['observations'].append(evidence(platform, coverage_ui_phase, 'coverage-theme-background'))
by_id['F13.03']['remaining'] = 'Theme 的预览、提交和重置均有真实终端背景色变化及偏好值的双平台证据；compact、timestamp、timeline 的全部渲染组合仍待验收。'
put('F16.02', 'verified_for_listed_contract', ['coverage-settings-voice-off'], phase=coverage_ui_phase)
by_id['F16.02']['qualification_note'] = '18 个后端禁用键逐一精确筛选无可操作行；显式 GROK_VOICE_MODE=0 后三项条件语音键同样隐藏，Vim 与 Permission 两项支持控件仍可筛选。原默认语音开启环境的错误前置条件评估保留。'

for platform, case, points, key, classification in [
    ('windows', 'coverage-program-code-search', ('F07.16',), 'counterexamples',
     'observed configured-budget timeout: outer program reports 10000ms timeout while its successful nested search records duration_ms=30048; no timeout or grader changed'),
    *[(platform, 'coverage-read-viewer', ('F12.07',), 'counterexamples',
       'product projection counterexample: canonical real read contains all 120 lines; End in the native read viewer stops around line 76 because its ACP text preview is bounded to 4000 characters') for platform in ('windows', 'linux')],
    *[(platform, 'coverage-unavailable-settings', ('F16.02',), 'excluded_evaluations',
       'harness precondition error: all 18 unavailable keys are absent, then it assumes voice is disabled although the actual default gate is enabled; independent public-env-pin profile preserves the original failure') for platform in ('windows', 'linux')],
    ('windows', 'coverage-theme-enum', ('F13.02', 'F13.03'), 'excluded_evaluations',
     'harness oracle error: whole-screen foreground color membership depends on transient text and chrome; restored Night background is RGB(20,20,20), independently measured in a new profile'),
    ('linux', 'coverage-markdown-raw', ('F12.06',), 'excluded_evaluations',
     'harness assumes exactly two search matches; real Thinking adds a third match and screen reports 1/3; independent profile selects the last real assistant match without changing the original prompt'),
]:
    relative = f'{platform}/{coverage_phase}/{case}.json'
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report['panicked'] and not report['assertions_finished'] and report['config_unchanged']
    item = {'platform': platform, 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
            'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
            'classification': classification}
    for point in points:
        by_id[point].setdefault(key, []).append(item)

for point in ('F07.13', 'F07.15', 'F07.16', 'F12.06', 'F12.07', 'F13.02', 'F13.03', 'F16.02'):
    row = by_id[point]
    references = row['observations'] + row.get('counterexamples', []) + row.get('excluded_evaluations', [])
    linked_files = {observation['file'] for observation in row['observations']}
    for observation in references:
        platform = observation['platform']
        phase = observation['file'].split('/')[1]
        if not phase.startswith('coverage-20261030-'):
            continue
        for name in ('test-binary.json', 'harness-source.json'):
            relative = f'{platform}/{phase}/{name}'
            if relative in linked_files:
                continue
            linked_files.add(relative)
            raw = (CAMPAIGN / relative).read_bytes()
            data = json.loads(raw)
            pair = data.get('source_pair', data.get('runtime_pair'))
            assert pair == observation['source_pair']
            row['observations'].append({'platform': platform, 'file': relative,
                'sha256': hashlib.sha256(raw).hexdigest(), 'source_pair': pair,
                'native_test_provenance': True})

network_phase = 'network-20261030-provider-direct/reports'
download_cases = ['network-download-binary', 'network-download-existing', 'network-download-hash',
                  'network-download-size', 'network-download-404', 'network-download-private', 'network-download-truncated']
fetch_cases = ['network-fetch-main', 'network-fetch-pages', 'network-fetch-redirect', 'network-fetch-404',
               'network-fetch-429', 'network-fetch-timeout', 'network-fetch-private']
by_id['F07.09']['status'] = 'partial'
by_id['F07.09']['observations'] = [evidence('linux', network_phase, 'network-download-binary')]
for platform in ('windows', 'linux'):
    by_id['F07.09']['observations'].extend(evidence(platform, network_phase, case) for case in download_cases[1:])
by_id['F07.09']['remaining'] = 'WSL 二进制下载至中文含空格路径、65536 精确字节、SHA-256 和独立 shell 校验通过。两端既有文件拒绝、覆盖时哈希失败保留原字节、大小上限、404、私有地址拒绝和截断 HTTP 正文不留下半文件均通过。Windows 实际下载回执成功且独立主机观察核对全部字节，但后续 Get-Item 的 bash 调用到原定 420 秒仍无回执，完整成功工作流未通过。'
by_id['F07.12']['status'] = 'partial'
by_id['F07.12']['observations'] = [evidence(platform, network_phase, case) for platform in ('windows', 'linux')
                                 for case in fetch_cases if case != 'network-fetch-pages']
by_id['F07.12']['observations'].extend(evidence(platform, 'network-20261030-direct-pages/reports', 'network-fetch-pages-direct') for platform in ('windows', 'linux'))
by_id['F07.12']['observations'].append(evidence('linux', 'network-20261030-pagination/reports', 'network-fetch-pages-inline-read'))
by_id['F07.12']['remaining'] = '受控真实 HTTP 的 main 提取、Markdown 链接、原始/重定向来源及原生屏幕来源显示、Unicode 0..17 和 17..EOF 分页、404、429、1 秒超时、私有 URL 拒绝及失败后继续对话均双端通过。缓存全文的 read 在 WSL 通过，Windows 对工具返回的 a3s://tool-output 地址产生真实策略 denied；保留该组合缺陷，不将其改判为抓取全通过。'
by_id['F07.01']['remaining'] += ' 新增反例：Windows read 对 web_fetch 自己返回的缓存全文 URI 给出 tool_effect_effective_arguments_unapproved；WSL 同一实际缓存读取成功。'
by_id['F07.01']['observations'].append(evidence('linux', 'network-20261030-pagination/reports', 'network-fetch-pages-inline-read'))
by_id['F07.08']['remaining'] += ' Windows 新增实际卡住反例：下载后的独立 Get-Item 命令已有 bash prepared，默认 timeout 为 120000ms，但原定 420 秒观察内没有结果或 turn.complete；具体阻塞阶段尚未归因。'

for platform in ('windows', 'linux'):
    for point, cases in [('F07.09', download_cases), ('F07.12', fetch_cases)]:
        for case in cases:
            relative = f'{platform}/network-20261030-native/reports/{case}.json'
            raw = (CAMPAIGN / relative).read_bytes(); report = json.loads(raw)
            assert report['panicked'] and report['config_unchanged'] and not report['assertions_finished']
            assert not any(f['type'] == 'tool.prepared' for f in report['facts'])
            by_id[point].setdefault('excluded_evaluations', []).append({'platform': platform,
                'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
                'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
                'classification': 'harness HTTP_PROXY also captures the HTTPS model transport before any tool dispatch; separate NO_PROXY profile keeps the same original model, prompts, limits and assertions'})
    relative = f'{platform}/{network_phase}/network-fetch-pages.json'
    raw = (CAMPAIGN / relative).read_bytes(); report = json.loads(raw)
    assert report['panicked'] and report['config_unchanged']
    by_id['F07.12'].setdefault('excluded_evaluations', []).append({'platform': platform,
        'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
        'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
        'classification': 'harness requests one read for its JSON document and one for the artifact but counts both as one requested model call; the independent inline read case preserves this original failure'})
for phase, case, points, classification in [
    (network_phase, 'network-download-binary', ('F07.09', 'F07.08'),
     'actual download succeeds and independent host read confirms all 65536 bytes; subsequent bash Get-Item prepared has no result through the unchanged 420s deadline; precise stage unconfirmed'),
    ('network-20261030-pagination/reports', 'network-fetch-pages-inline-read', ('F07.01', 'F07.12'),
     'actual Windows artifact read receives outcome=denied and tool_effect_effective_arguments_unapproved for the URI the real web_fetch returns; the grader then attempts JSON parsing, but the underlying policy denial remains a product counterexample'),
]:
    relative = f'windows/{phase}/{case}.json'; raw = (CAMPAIGN / relative).read_bytes(); report = json.loads(raw)
    assert report['panicked'] and report['config_unchanged'] and not report['assertions_finished']
    item = {'platform': 'windows', 'file': relative, 'sha256': hashlib.sha256(raw).hexdigest(),
            'source_pair': next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair'),
            'classification': classification}
    for point in points:
        by_id[point].setdefault('counterexamples', []).append(item)
for point in ('F07.01', 'F07.08', 'F07.09', 'F07.12'):
    row = by_id[point]
    references = row['observations'] + row.get('counterexamples', []) + row.get('excluded_evaluations', [])
    linked = {item['file'] for item in row['observations']}
    for item in references:
        platform = item['platform']; phase = item['file'].split('/')[1]
        if not phase.startswith('network-20261030-'):
            continue
        for name in ('test-binary.json', 'harness-source.json'):
            relative = f'{platform}/{phase}/{name}'
            if relative in linked:
                continue
            linked.add(relative); raw = (CAMPAIGN / relative).read_bytes(); data = json.loads(raw)
            pair = data.get('source_pair', data.get('runtime_pair')); assert pair == item['source_pair']
            row['observations'].append({'platform': platform, 'file': relative,
                'sha256': hashlib.sha256(raw).hexdigest(), 'source_pair': pair, 'native_test_provenance': True})

counts = {status: sum(row['status'] == status for row in rows) for status in sorted({row['status'] for row in rows})}
result = {'schema': 'aster.tui.checkpoints.v1', 'complete': False, 'model': 'kimi/k3.1-pro', 'checkpoint_count': len(rows), 'domain_count': 16, 'claim': '仅所列合同和所链接运行有证据；a891 完整 35 项批次为 Windows 35/35、Linux 35/35，F02 六项所列合同全部验证（含信任、ACL、路径隔离及交互式相对 cwd）；历史反例与独立复跑均保留。新增边界单独绑定其源码，不替代完整批次或 153 项验收。', 'status_counts': counts, 'verified_percent': round(counts.get('verified_for_listed_contract', 0) * 100 / len(rows), 2), 'checkpoints': rows}
(CAMPAIGN / 'CHECKPOINTS.json').write_bytes((json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
reopened = json.loads((CAMPAIGN / 'CHECKPOINTS.json').read_text(encoding='utf-8'))
assert [row['id'] for row in reopened['checkpoints']] == [row['id'] for row in rows]
print(json.dumps({'checkpoints':len(rows),'domains':16,'successful_evidence_files':len(cache),'complete':False}))
