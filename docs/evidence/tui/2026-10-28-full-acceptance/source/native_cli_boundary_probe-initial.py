"""Native CLI boundaries, original ACL, no operator configuration copies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

p = argparse.ArgumentParser()
p.add_argument('--repo-root', type=Path, required=True)
p.add_argument('--config', type=Path, required=True)
p.add_argument('--pair', type=Path, required=True)
p.add_argument('--older-pair', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
repo = a.repo_root.resolve()
config = a.config.resolve(strict=True)
parent = (repo / '.tmp').resolve(strict=True)
assert parent.is_relative_to(repo)
pair = json.loads(a.pair.read_text())
older = json.loads(a.older_pair.read_text())
assert pair['source_pair'].split(':')[1] != older['source_pair'].split(':')[1]
native_windows = os.name == 'nt'
assert native_windows or sys.platform.startswith('linux')
suffix = '.exe' if native_windows else ''
binary = Path(pair['folder']) / ('aster' + suffix)
sidecar = Path(pair['folder']) / ('a3s-code-acp' + suffix)
old_sidecar = Path(older['folder']) / ('a3s-code-acp' + suffix)
assert all(path.resolve().is_relative_to(repo) for path in (binary, sidecar, old_sidecar))
digest = lambda data: hashlib.sha256(data).hexdigest()
before = digest(config.read_bytes())
with binary.open('rb') as stream:
    header = stream.read(4)
assert header[:2] == b'MZ' if native_windows else header == b'\x7fELF'
for path in (binary, sidecar):
    assert subprocess.check_output([str(path), '--build-id'], text=True).strip() == pair['source_pair']

rows = []
report = dict(schema='aster.tui.native-cli-boundaries.v1', platform=sys.platform,
              model='kimi/k3.1-pro', source_pair=pair['source_pair'], cases=rows,
              script_sha256=digest(Path(__file__).read_bytes()))
a.output.parent.mkdir(parents=True, exist_ok=True)


def save():
    a.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def facts(workspace):
    out = []
    files = list((workspace / '.a3s').rglob('*.jsonl'))
    assert len(files) <= 64
    for path in files:
        data = path.read_bytes()
        assert len(data) <= 4 * 1024 * 1024
        for line in data.decode('utf-8').splitlines():
            row = json.loads(line)
            if 'type' in row and 'seq' in row:
                out.append(row)
    return out


for kind in ('headless-json', 'missing-companion', 'mismatched-companion'):
    temp = tempfile.TemporaryDirectory(prefix='aster-cli-boundary-', dir=parent)
    root = Path(temp.name).resolve()
    assert root.is_relative_to(parent)
    try:
        workspace = root / 'workspace 中文 with spaces'
        home = root / 'home'
        for path in (workspace, home / 'grok', home / 'AppData/Local', home / 'AppData/Roaming'):
            path.mkdir(parents=True, exist_ok=True)
        exe = binary
        if kind != 'headless-json':
            bundle = root / 'bundle'
            bundle.mkdir()
            exe = bundle / binary.name
            shutil.copy2(binary, exe)
            if kind == 'mismatched-companion':
                shutil.copy2(old_sidecar, bundle / sidecar.name)
        env = {key: os.environ[key] for key in ('PATH', 'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT',
               'ProgramFiles', 'ProgramFiles(x86)', 'ProgramData', 'TEMP', 'TMP') if key in os.environ}
        env.update(HOME=str(home), USERPROFILE=str(home), GROK_HOME=str(home / 'grok'),
                   APPDATA=str(home / 'AppData/Roaming'), LOCALAPPDATA=str(home / 'AppData/Local'),
                   A3S_CONFIG=str(config), A3S_DISABLE_CC_SWITCH='1', A3S_DISABLE_GROK_ACCOUNT='1',
                   A3S_DISABLE_CURSOR_ACCOUNT='1', TERM='xterm-256color')
        marker = 'CLI-NONTTY-' + uuid.uuid4().hex
        prompt = f'Reply only {marker}. Do not call any tool. 中文 e\u0301'
        command = [str(exe), '--cwd', str(workspace), '--no-plan', '--model', 'kimi/k3.1-pro',
                   '--verbatim', '--output-format', 'json', '--single', prompt]
        started = time.monotonic()
        kw = dict(creationflags=subprocess.CREATE_NO_WINDOW) if native_windows else dict(start_new_session=True)
        child = subprocess.Popen(command, cwd=workspace, env=env, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kw)
        timed_out = False
        try:
            stdout, stderr = child.communicate(timeout=180)
        except subprocess.TimeoutExpired:
            timed_out = True
            if native_windows:
                subprocess.run(['taskkill', '/PID', str(child.pid), '/T', '/F'], capture_output=True)
            else:
                os.killpg(child.pid, signal.SIGKILL)
            stdout, stderr = child.communicate(timeout=15)
        observed = facts(workspace)
        item = dict(case=kind, exit_code=child.returncode, timed_out=timed_out,
                    seconds=round(time.monotonic() - started, 3), stdout_sha256=digest(stdout),
                    stderr_sha256=digest(stderr), tool_calls=sum(f['type'] == 'tool.prepared' for f in observed),
                    model_turns=sum(f['type'] == 'model.turn' for f in observed))
        try:
            assert not timed_out
            assert digest(config.read_bytes()) == before
            if kind == 'headless-json':
                result = json.loads(stdout.decode('utf-8'))
                assert child.returncode == 0 and result['text'].strip() == marker
                assert result['stopReason'] == 'end_turn' and result['sessionId'] and result['requestId']
                users = [f['payload']['text'] for f in observed if f['type'] == 'user.message']
                assert users == [prompt]
                assert any(f['type'] == 'model.turn' and (f['payload'].get('prompt_tokens') or 0) > 0 for f in observed)
                assert sum(f['type'] == 'turn.complete' for f in observed) == 1 and item['tool_calls'] == 0
                item['exact_user_message'] = item['exact_json_response'] = True
            else:
                assert child.returncode != 0 and item['model_turns'] == item['tool_calls'] == 0
                diagnostic = b'no a3s-code-acp beside the pager' if kind == 'missing-companion' else b'different build identities'
                assert diagnostic in stdout + stderr
                item['explicit_refusal'] = True
            item['result'] = 'pass'
        except (AssertionError, KeyError, ValueError, UnicodeError):
            item['result'] = 'fail'
        rows.append(item)
        save()
    finally:
        assert root.resolve().is_relative_to(parent) and parent.is_relative_to(repo)
        temp.cleanup()

report['config_unchanged'] = digest(config.read_bytes()) == before
report['all_passed'] = report['config_unchanged'] and all(row['result'] == 'pass' for row in rows)
save()
print(json.dumps(dict(platform=sys.platform, source_pair=pair['source_pair'], cases=rows)))
assert report['all_passed'], 'retained native CLI boundary failure'
