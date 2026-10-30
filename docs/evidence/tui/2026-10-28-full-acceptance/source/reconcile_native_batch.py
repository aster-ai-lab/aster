"""Bind every completed native batch report, including failed assertions."""
import argparse
import hashlib
import json
from pathlib import Path
import re

p = argparse.ArgumentParser()
p.add_argument('--phase', required=True)
p.add_argument('--digest', required=True)
p.add_argument('--output', required=True)
a = p.parse_args()
root = Path(__file__).resolve().parent.parent
rows = []
for platform in ('windows', 'linux'):
    folder = root / platform / a.phase
    raw = (folder / 'run.log').read_bytes()
    results = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;.*?finished in ([0-9.]+)s', raw.decode('utf-8-sig'))
    assert len(results) == 1
    state, passed, failed, ignored, seconds = results[0]
    assert int(passed) + int(failed) == 35 and int(ignored) == 0
    pair = json.loads((folder / 'pair.json').read_text(encoding='utf-8-sig'))
    assert pair['source_pair'].split(':')[1] == a.digest
    expected = {path.name for path in (root / platform / 'full-35-qualification-final').glob('*.json') if path.name != 'pair.json'}
    files = [path for path in folder.glob('*.json') if path.name != 'pair.json']
    assert {path.name for path in files} == expected
    reports = []
    for path in sorted(files):
        data_bytes = path.read_bytes()
        data = json.loads(data_bytes)
        assert data['model'] == 'kimi/k3.1-pro' and data['config_unchanged']
        identity = next(item['detail'] for item in data['oracles'] if item.get('name') == 'source-pair')
        assert identity == pair['source_pair']
        reports.append(dict(case=data['case'], file=path.relative_to(root).as_posix(),
                            sha256=hashlib.sha256(data_bytes).hexdigest(),
                            assertions_finished=data['assertions_finished'], panicked=data['panicked']))
    if state == 'ok':
        assert all(item['assertions_finished'] and not item['panicked'] for item in reports)
    rows.append(dict(platform=platform, source_pair=pair['source_pair'], binaries=pair['files'],
                     entrypoints_passed=int(passed), entrypoints_failed=int(failed), seconds=float(seconds),
                     run_log_sha256=hashlib.sha256(raw).hexdigest(), case_reports=reports))
result = dict(schema='aster.tui.native-final-runs.v1', model='kimi/k3.1-pro',
              source_pair_digest=a.digest, phase=a.phase,
              all_entrypoints_passed=all(item['entrypoints_failed'] == 0 for item in rows),
              checkpoint_count=153, complete_153=False, whole_pager_qualified=False, platforms=rows)
out = root / a.output
assert out.resolve().is_relative_to(root.resolve()) and not out.exists()
out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
reopened = json.loads(out.read_text(encoding='utf-8'))
assert reopened == result
print(json.dumps({item['platform']: dict(passed=item['entrypoints_passed'], failed=item['entrypoints_failed'],
                                       seconds=item['seconds'], reports=len(item['case_reports'])) for item in rows}))
