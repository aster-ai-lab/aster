from pathlib import Path
import json
import re
import subprocess
import sys
import time

out = Path('.tmp/mise-project-uninstall-final-checks.json')
record = {'status': 'running', 'startedAtUnixSeconds': time.time(), 'checks': []}
checks = [
    ('tool-tests', ['cargo', 'test', '--locked', '-p', 'aster-use-tool-acquire']),
    ('tool-clippy', ['cargo', 'clippy', '--locked', '-p', 'aster-use-tool-acquire', '--all-targets', '--no-deps']),
    ('pager-tests', ['cargo', 'test', '--locked', '-p', 'aster-pager', '--lib', 'env_manager_cmd::tests', '--', '--test-threads=1']),
    ('cli-test-build', ['cargo', 'test', '--locked', '-p', 'aster', '--test', 'env_manager', '--no-run']),
    ('cli-clippy', ['cargo', 'clippy', '--locked', '-p', 'aster', '--all-targets', '--no-deps']),
    ('pager-clippy', ['cargo', 'clippy', '--locked', '-p', 'aster-pager', '--all-targets', '--no-deps']),
    ('paired-build', ['cargo', 'build', '--locked', '-p', 'aster', '-p', 'aster-acp']),
]
old = json.loads(Path('.tmp/mise-project-uninstall-pager-findings.json').read_text())
old_locations = {row['primaryLocation'] for row in old['findings']}


def save():
    out.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')


for name, args in checks:
    log = Path('.tmp') / ('mise-project-uninstall-final-' + name + '.log')
    row = {'name': name, 'argv': args, 'log': str(log), 'status': 'running',
           'startedAtUnixSeconds': time.time()}
    record['checks'].append(row)
    save()
    with log.open('w') as stream:
        code = subprocess.run(args, stdout=stream, stderr=subprocess.STDOUT).returncode
    row.update(exitCode=code, status='passed' if code == 0 else 'failed', finishedAtUnixSeconds=time.time())
    if name == 'pager-clippy' and code == 101:
        findings = []
        for block in re.split(r'(?m)(?=^error: )', log.read_text())[1:]:
            if block.startswith('error: could not compile'):
                continue
            location = re.search(r'--> (.+?):(\d+):(\d+)', block)
            if location:
                findings.append({'diagnostic': block.splitlines()[0],
                                 'primaryLocation': f'--> {location[1]}:{location[2]}:{location[3]}'})
        locations = {row['primaryLocation'] for row in findings}
        row['findings'] = findings
        row['samePriorLocations'] = locations == old_locations
        row['newPrimaryLocations'] = sorted(locations - old_locations)
        if findings and locations <= old_locations:
            row['status'] = 'known_existing_failure'
    save()
    if code and row['status'] != 'known_existing_failure':
        record['status'] = 'failed'
        save()
        sys.exit(code)
record.update(status='completed_with_known_pager_failure' if any(row['exitCode'] for row in record['checks']) else 'passed',
              finishedAtUnixSeconds=time.time())
save()
print(record['status'])
