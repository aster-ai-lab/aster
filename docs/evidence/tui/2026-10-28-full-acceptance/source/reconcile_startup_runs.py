"""Re-open the exact native CLI reports and frozen graders before landing."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent
digest = '189982670423c6c055335d5e701b491d295583e5'
expected = ['headless-json', 'headless-relative-cwd', 'missing-companion',
            'mismatched-companion', 'unexecutable-companion', 'missing-acl',
            'invalid-acl', 'unknown-model']
rows = []
for platform in ('windows', 'linux'):
    pair = json.loads((root / platform / 'startup-checked/pair.json').read_text(encoding='utf-8-sig'))
    assert pair['source_pair'].split(':')[1] == digest
    references = []
    for phase, cases, script in [
        ('native-cli-startup-errors', expected, 'native_cli_boundary_probe-tool-receipt-expectation.py'),
        ('native-cli-confirmation-cancel', ['scheduler-confirmation-cancel'], 'native_cli_boundary_probe-882cb4fc4ab2.py'),
    ]:
        path = root / platform / phase / 'report.json'
        raw = path.read_bytes()
        data = json.loads(raw)
        assert data['config_unchanged'] and data['all_passed']
        assert data['source_pair'] == pair['source_pair']
        assert [item['case'] for item in data['cases']] == cases
        assert data['script_sha256'] == hashlib.sha256((root / 'source' / script).read_bytes()).hexdigest()
        assert all(item['result'] == 'pass' and not item['timed_out'] for item in data['cases'])
        references.append(dict(file=path.relative_to(root).as_posix(), sha256=hashlib.sha256(raw).hexdigest(),
                               grader='source/' + script, cases=cases, results=data['cases']))
    rows.append(dict(platform=platform, source_pair=pair['source_pair'], binaries=pair['files'],
                     native_cli_cases=9, references=references))
result = dict(schema='aster.tui.startup-qualification.v1', source_pair_digest=digest,
              model='kimi/k3.1-pro', configuration_faults_use_explicit_synthetic_fixtures=True,
              native_cli_cases_per_platform=9, platforms=rows,
              complete_153=False, whole_pager_qualified=False,
              latest_full_35_batch='RUNS-43CE.json')
out = root / 'STARTUP-RUNS.json'
out.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
assert json.loads(out.read_text(encoding='utf-8')) == result
print(json.dumps({'source_pair_digest': digest, 'platforms': 2, 'native_cli_cases_per_platform': 9, 'complete_153': False}))
