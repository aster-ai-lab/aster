"""Bind the completed 35-entry native runs to their immutable source pair."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
PAIR = '32b8c01e8224e33564c3f38d03c84dffcd465d95'
rows = []
for platform in ('windows', 'linux'):
    folder = ROOT / platform / 'full-35-qualification-final'
    raw = (folder / 'run.log').read_bytes()
    text = raw.decode('utf-8-sig')
    result = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;.*?finished in ([0-9.]+)s', text)
    assert len(result) == 1, (platform, 'run is not complete')
    state, passed, failed, ignored, seconds = result[0]
    assert int(passed) + int(failed) == 35 and int(ignored) == 0
    pair = json.loads((folder / 'pair.json').read_text())
    assert pair['source_pair'].split(':')[1] == PAIR
    expected = {path.name for path in (ROOT / platform / 'full-35-completion-facts').glob('*.json') if path.name != 'pair.json'}
    files = [path for path in folder.glob('*.json') if path.name != 'pair.json']
    assert {path.name for path in files} == expected, (platform, 'missing or extra case report')
    reports = []
    for path in sorted(files):
        data_bytes = path.read_bytes()
        report = json.loads(data_bytes)
        assert report['model'] == 'kimi/k3.1-pro' and report['config_unchanged']
        identity = next(row['detail'] for row in report['oracles'] if row.get('name') == 'source-pair')
        assert identity == pair['source_pair']
        reports.append(dict(case=report['case'], file=path.relative_to(ROOT).as_posix(),
                            sha256=hashlib.sha256(data_bytes).hexdigest(),
                            assertions_finished=report['assertions_finished'], panicked=report['panicked']))
    if state == 'ok':
        assert all(report['assertions_finished'] and not report['panicked'] for report in reports)
    rows.append(dict(platform=platform, source_pair=pair['source_pair'], binaries=pair['files'],
                     entrypoints_passed=int(passed), entrypoints_failed=int(failed), seconds=float(seconds),
                     run_log_sha256=hashlib.sha256(raw).hexdigest(), case_reports=reports))
result = dict(schema='aster.tui.native-final-runs.v1', model='kimi/k3.1-pro', source_pair_digest=PAIR,
              all_entrypoints_passed=all(row['entrypoints_failed'] == 0 for row in rows),
              checkpoint_count=153, complete_153=False, whole_pager_qualified=False, platforms=rows)
(ROOT / 'FINAL-RUNS.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({row['platform']: dict(passed=row['entrypoints_passed'], failed=row['entrypoints_failed'],
                                      seconds=row['seconds'], reports=len(row['case_reports'])) for row in rows}))
assert result['all_entrypoints_passed'], 'retained failures prevent claiming a fully passing live batch'
