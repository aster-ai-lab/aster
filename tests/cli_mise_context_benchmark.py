"""Measure actual path-selection commands; no release-efficiency claim."""
import hashlib
import json
import argparse
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--aster', type=Path, default=root / 'target/debug/aster')
parser.add_argument('--profile', choices=['dev', 'release'], default='dev')
parser.add_argument('--output', type=Path, required=True)
options = parser.parse_args()
if os.name != 'posix':
    parser.error('This fixture requires native POSIX shell execution; Windows acceptance uses its native harness.')
output = options.output
assert not output.exists(), f'Preserve existing benchmark: {output}'
cli = options.aster.resolve()
mise = (root / '.tmp/mise-replacement/reference/mise').resolve()
reference = json.loads((root / 'specs/031-cli-mise-replacement/contracts/mise-context-reference.json').read_text())


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


identities = {'aster': digest(cli), 'mise': digest(mise)}
assert identities['mise'] == reference['binary_sha256']
results = []
report = {'host': {'system': platform.system(), 'machine': platform.machine()},
          'aster_profile': options.profile,
          'reference_tag': reference['tag'], 'binary_sha256': identities,
          'input': 'node@path:./SDK node; actual external shell program prints external-node',
          'os_cache_cold_controlled': False, 'background_load_controlled': False, 'full_goal_complete': False,
          'benchmark_source_sha256': digest(Path(__file__)),
          'summary': [], 'commands': results,
          'limits': ['Only native path-selection overhead is measured',
                     'Fresh owned state does not imply cold filesystem/executable caches',
                     'Host load is uncontrolled; this is diagnostic, not isolated performance certification',
                     'No managed download, runtime workload, other backend or native platform acceptance']}


def write_report(status):
    report['status'] = status
    report['summary'] = []
    for tool in ['aster', 'mise']:
        for state in ['fresh_owned_state', 'repeated_owned_state']:
            samples = [entry['elapsed_ms'] for entry in results
                       if entry['tool'] == tool and entry['state'] == state and entry['status'] == 'passed']
            report['summary'].append({'tool': tool, 'state': state, 'samples': len(samples),
                                      'complete_samples': len(samples) == 5,
                                      'median_ms': round(statistics.median(samples), 3) if samples else None,
                                      'max_ms': max(samples) if samples else None})
    output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    assert json.loads(output.read_text()) == report


output.parent.mkdir(parents=True, exist_ok=True)
try:
    with tempfile.TemporaryDirectory(prefix='aster-context-benchmark-') as temporary:
        for sample in range(5):
            for name, binary in [('aster', cli), ('mise', mise)]:
                base = Path(temporary) / str(sample) / name
                cwd = base / '项目 with spaces'
                program = cwd / 'SDK node/bin/node'
                program.parent.mkdir(parents=True)
                program.write_text("#!/bin/sh\nprintf '%s\\n' 'external-node'\n", encoding='utf-8')
                program.chmod(0o755)
                environment = {'PATH': '/usr/bin:/bin', 'HOME': str(base / 'home'), 'NO_COLOR': '1'}
                if name == 'aster':
                    environment['ASTER_HOME'] = str(base / 'aster-home')
                    args = ['--no-auto-update', 'exec', '--offline', '--tool', 'node@path:./SDK node', '--', 'node']
                    deadline = 20
                else:
                    environment.update({
                        'MISE_CONFIG_DIR': str(base / 'config'), 'MISE_DATA_DIR': str(base / 'data'),
                        'MISE_CACHE_DIR': str(base / 'cache'), 'MISE_SYSTEM_CONFIG_DIR': str(base / 'system'),
                        'MISE_GLOBAL_CONFIG_FILE': str(base / 'config/global.toml'),
                        'MISE_GLOBAL_CONFIG_ROOT': str(base / 'home'), 'MISE_PARANOID': '1', 'MISE_YES': '1',
                    })
                    args = ['exec', 'node@path:./SDK node', '--', 'node']
                    deadline = 25
                for state in ['fresh_owned_state', 'repeated_owned_state']:
                    record = {'tool': name, 'sample': sample, 'state': state,
                              'deadline_seconds': deadline, 'command': [str(binary), *args],
                              'status': 'running', 'exit': None, 'stdout': '', 'elapsed_ms': None}
                    results.append(record)
                    write_report('running')
                    start = time.perf_counter_ns()
                    try:
                        result = subprocess.run([str(binary), *args], cwd=cwd, env=environment,
                                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                                stderr=subprocess.PIPE, timeout=deadline)
                    except subprocess.TimeoutExpired as error:
                        record.update(status='timeout', elapsed_ms=round((time.perf_counter_ns() - start) / 1_000_000, 3),
                                      stdout_hex=(error.stdout or b'').hex(), stderr_hex=(error.stderr or b'').hex())
                        raise
                    record.update(status='passed' if result.returncode == 0 and result.stdout == b'external-node\n' else 'failed',
                                  elapsed_ms=round((time.perf_counter_ns() - start) / 1_000_000, 3),
                                  exit=result.returncode, stdout=result.stdout.decode('utf-8', errors='replace'),
                                  stdout_hex=result.stdout.hex(), stderr_hex=result.stderr.hex())
                    write_report('running')
                    assert record['status'] == 'passed', record
                if name == 'aster':
                    assert not (base / 'aster-home/tools/receipts').exists()
                    assert not (base / 'aster-home/tools/current').exists()
                    assert not (cwd / '.aster/env.acl').exists()
    report['observed_final_binary_sha256'] = {'aster': digest(cli), 'mise': digest(mise)}
    assert identities == report['observed_final_binary_sha256']
    assert len(results) == 20 and all(entry['status'] == 'passed' for entry in results)
except BaseException as error:
    report['failure'] = {'type': type(error).__name__, 'message': str(error)}
    write_report('failed')
    raise
write_report('complete')
print(json.dumps({'output': str(output), 'summary': report['summary']}))
