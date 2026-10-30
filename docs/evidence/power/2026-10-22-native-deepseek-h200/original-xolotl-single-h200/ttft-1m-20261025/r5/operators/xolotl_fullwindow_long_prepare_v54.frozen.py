"""CPU-only exact tokenizer preparation; no service credentials and no GPU inference."""
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from xolotl_fullwindow_long_fixture_v54 import build_fixture

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'prepare-plan.json').read_text())
SOURCE = Path(PLAN['source_public_root'])
STATE = dict(schema='aster-xolotl-fullwindow-long-prepare-v54', unix=time.time(), phase='preflight',
             gpu_executed=False, credentials_copied=False)


def save():
    tmp = ROOT / 'prepare-result.tmp'
    tmp.write_text(json.dumps(STATE, indent=2) + '\n')
    tmp.replace(ROOT / 'prepare-result.json')


try:
    os.umask(0o077)
    save()
    tests = subprocess.run([sys.executable, str(ROOT / 'test_xolotl_fullwindow_long_fixture_v54.py'), '-v'],
                           cwd=ROOT, capture_output=True, text=True, timeout=30)
    (ROOT / 'unit-tests.log').write_text(tests.stdout + tests.stderr)
    if tests.returncode:
        raise RuntimeError('full-window long-output acceptance unit tests failed')
    STATE['acceptance_unit_methods_passed'] = 6
    if SOURCE.resolve() != SOURCE or SOURCE.stat().st_uid != os.getuid():
        raise RuntimeError('exact public CPU source owner differs')
    api = SOURCE / 'source/tools/v41/serve/serve_ds41.py'
    if api.is_symlink() or hashlib.sha256(api.read_bytes()).hexdigest() != PLAN['api_sha256']:
        raise RuntimeError('exact full-window fixture tokenizer source differs')
    sys.path[:0] = [str(SOURCE / 'python-deps-v2'), str(SOURCE / 'source'),
                   str(SOURCE / 'source/tools/v41/serve')]
    import serve_ds41 as D
    vocab = json.loads((SOURCE / 'tokenizer/vocab.json').read_text())
    tokens = [None] * len(vocab)
    for token, index in vocab.items():
        tokens[index] = token
    tokenizer = D.Ds41Tokenizer(tokens, (SOURCE / 'tokenizer/merges.txt').read_text().split('\n'),
                               json.loads((SOURCE / 'tokenizer/token_type.json').read_text()))
    text, metadata = build_fixture(tokenizer, D.Ds41Template())
    with (ROOT / 'public-fixture.txt').open('xb') as output:
        output.write(text.encode())
    with (ROOT / 'fixture-metadata.json').open('x') as output:
        output.write(json.dumps(metadata, indent=2) + '\n')
    STATE.update(phase='passed', fixture=metadata, finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
