"""Check matrix fidelity, retained evidence bytes and actual dual-platform passes."""
import hashlib
import argparse
import json
from pathlib import Path
import re

campaign = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--output', default='source/coverage-20261030-index-audit.json')
args = parser.parse_args()
path = campaign / 'CHECKPOINTS.json'
raw = path.read_bytes()
index = json.loads(raw)
expected = [match[1] for line in (campaign / 'OUTCOME.md').read_text(encoding='utf-8').splitlines()
            if (match := re.fullmatch(r'\| (F\d{2}\.\d{2}) \| (.+) \|', line))]
rows = index['checkpoints']
assert [row['id'] for row in rows] == expected
assert len(expected) == len(set(expected)) == index['checkpoint_count'] == 153
assert len({row['id'].split('.')[0] for row in rows}) == index['domain_count'] == 16
assert not index['complete'] and b'\r' not in raw
checked = {}
verified = 0
for row in rows:
    passing = set()
    for category in ('observations', 'counterexamples', 'excluded_evaluations'):
        for observation in row.get(category, []):
            file = observation['file']
            if file not in checked:
                data = (campaign / file).read_bytes()
                checked[file] = (hashlib.sha256(data).hexdigest(), data)
            digest, data = checked[file]
            assert digest == observation['sha256'], file
            if category == 'observations' and file.endswith('.json'):
                report = json.loads(data)
                if report.get('assertions_finished'):
                    assert not report['panicked'] and report['config_unchanged']
                    assert report['model'] == 'kimi/k3.1-pro'
                    passing.add(observation['platform'])
                elif (row['status'] == 'verified_for_listed_contract'
                      and report.get('schema') == 'aster.tui.native-cli-boundaries.v1'):
                    assert report['all_passed'] and report['config_unchanged']
                    assert report['model'] == 'kimi/k3.1-pro' and report['cases']
                    cases = {case['case']: case for case in report['cases']}
                    required = observation.get('native_cases', [observation['native_case']] if 'native_case' in observation else [])
                    assert required and all(name in cases for name in required)
                    for name in required:
                        case = cases[name]
                        assert case['result'] == 'pass' and not case['timed_out']
                        if 'checks' in case:
                            assert case['checks'] and all(case['checks'].values())
                        elif 'explicit_refusal' in case:
                            assert case['explicit_refusal'] and case['exit_code'] == 1
                            assert case['tool_calls'] == case['model_turns'] == 0
                        else:
                            assert case['case'] == 'scheduler-confirmation-cancel'
                            assert case['exit_code'] == 0 and case['tool_calls'] == 0
                            assert case['model_requested_tools'] == ['write']
                            assert case['confirmation_answers'] == [False]
                            assert case['confirmation_cancelled_before_execution'] and case['no_file_effect']
                    passing.add(observation['platform'])
    if row['status'] == 'verified_for_listed_contract':
        assert passing == {'windows', 'linux'}, row['id']
        verified += 1
counts = {status: sum(row['status'] == status for row in rows)
          for status in sorted({row['status'] for row in rows})}
assert counts == index['status_counts']
assert index['verified_percent'] == round(verified * 100 / 153, 2)
result = {'schema': 'aster.tui.index-audit.v1', 'index_sha256': hashlib.sha256(raw).hexdigest(),
          'checkpoint_count': 153, 'domain_count': 16, 'status_counts': counts,
          'verified_with_actual_passing_cases_on_both_platforms': verified,
          'referenced_files_checked': len(checked), 'all_referenced_hashes_match': True,
          'complete_153': False}
destination = campaign / args.output
assert not destination.exists(), 'retain earlier audit'
destination.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text()) == result
print(json.dumps(result))
