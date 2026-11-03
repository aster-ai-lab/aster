from pathlib import Path
import datetime
import hashlib
import json
import subprocess
import time

root = Path.cwd()
previous = root / '.tmp/plugin-registry/babeldoc-measurement'
output = root / '.tmp/plugin-registry/babeldoc-compatible-v2'
assert not output.exists()
output.mkdir()
(output / 'requirements.in').write_text('babeldoc-stream==0.6.4.post4\nbitstring>=4.3.0,<5\n', encoding='utf-8')
constraints = []
for line in (previous / 'requirements.lock').read_text().splitlines():
    if line and not line.startswith((' ', '#')):
        requirement = line.removesuffix(' \\')
        name, separator, _ = requirement.partition('==')
        assert separator, line
        if name not in ['bitstring', 'tibs']:
            constraints.append(requirement)
(output / 'constraints.txt').write_text('\n'.join(constraints) + '\n', encoding='utf-8')
base = ['uv', '--no-config', '--cache-dir', str(previous / 'cache'), 'pip']
commands = [
    base + ['compile', str(output / 'requirements.in'), '--constraint', str(output / 'constraints.txt'), '--python-version', '3.12', '--python-platform', 'aarch64-apple-darwin', '--generate-hashes', '--output-file', str(output / 'requirements.lock')],
    base + ['install', '--target', str(output / 'site-packages'), '--python-version', '3.12', '--python-platform', 'aarch64-apple-darwin', '--only-binary', ':all:', '--require-hashes', '--index-url', 'https://pypi.org/simple', '-r', str(output / 'requirements.lock')],
]
checks = []
for index, argv in enumerate(commands):
    started = time.monotonic()
    with (output / f'prepare-{index}.log').open('w') as log:
        result = subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT)
    checks.append({'argv': argv, 'exitCode': result.returncode, 'seconds': time.monotonic() - started})
    report = {
        'capturedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'checks': checks,
        'previousRequirementsSha256': hashlib.sha256((previous / 'requirements.lock').read_bytes()).hexdigest(),
        'compatibilityReason': 'BabelDOC imports BitStream; bitstring 5 removed it.',
        'primarySource': 'https://bitstring.readthedocs.io/en/stable/upgrading_to_version_5.html',
        'otherPreviouslyResolvedVersionsConstrained': True,
        'aixBuilt': False, 'published': False,
    }
    (output / 'preparation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(index, result.returncode, flush=True)
    assert result.returncode == 0
