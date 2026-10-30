"""Apply a frozen public overlay and run both modes before any GPU deployment."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parent
STATE = dict(phase='preflight', unix=time.time(), gpu_executed=False, tests=[])
def save():
    p = ROOT / 'cpu-preflight-result.tmp'
    p.write_text(json.dumps(STATE, indent=2) + '\n')
    p.replace(ROOT / 'cpu-preflight-result.json')
try:
    save()
    os.nice(19)
    manifest = json.loads((ROOT / 'public-manifest.json').read_text())
    overlay = json.loads((ROOT / 'overlay-plan.json').read_text())
    api = ROOT / 'source/tools/v41/serve/serve_ds41.py'
    if hashlib.sha256(api.read_bytes()).hexdigest() != overlay['upstream_sha256']:
        raise RuntimeError('exact original API parent differs')
    patched = (ROOT / 'overlay/serve_ds41.py').read_bytes()
    helper = (ROOT / 'overlay/aster_literal_scan.py').read_bytes()
    if hashlib.sha256(patched).hexdigest() != overlay['patched_sha256'] or hashlib.sha256(helper).hexdigest() != overlay['helper_sha256']:
        raise RuntimeError('exact frozen API/scanner bytes differ')
    api.write_bytes(patched)
    target = api.with_name('aster_literal_scan.py')
    with target.open('xb') as output:
        output.write(helper)
    rows = [row for row in manifest['files'] if row['file'] == 'source/tools/v41/serve/serve_ds41.py']
    if len(rows) != 1:
        raise RuntimeError('unique qualified API source row differs')
    rows[0].update(bytes=len(patched), sha256=overlay['patched_sha256'])
    manifest['files'].append(dict(file='source/tools/v41/serve/aster_literal_scan.py', bytes=len(helper), sha256=overlay['helper_sha256']))
    manifest.update(previous_overlay=manifest.get('overlay'), overlay=overlay, schema='aster-public-exact-literal-api-v148')
    (ROOT / 'public-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    deps = ROOT / 'python-deps-preflight'
    deps.mkdir(mode=0o700)
    total = 0
    for row in manifest['wheels']:
        with zipfile.ZipFile(ROOT / row['file']) as archive:
            for member in archive.infolist():
                name = PurePosixPath(member.filename)
                total += member.file_size
                if name.is_absolute() or '..' in name.parts or member.file_size > 8*1024**2 or total > 24*1024**2 or (member.external_attr>>16)&0o170000 == 0o120000:
                    raise RuntimeError('unsafe pinned wheel member')
                dest = deps.joinpath(*name.parts)
                if member.is_dir():
                    dest.mkdir(mode=0o700, parents=True, exist_ok=True)
                    continue
                dest.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with dest.open('xb') as output:
                    output.write(archive.read(member))
    env = {k:v for k,v in os.environ.items() if not k.startswith(('STRATA_', 'ASTER_XQ_', 'PYTHONPATH'))}
    env.update(PYTHONPATH=str(deps)+os.pathsep+str(ROOT/'source')+os.pathsep+str(api.parent),
        PYTHONDONTWRITEBYTECODE='1', STRATA_TEXT_LOG='0', CUDA_VISIBLE_DEVICES='', ASTER_XQ_EXACT_TOKEN_CACHE='1')
    tests = [('upstream-security', [sys.executable, '-m', 'unittest', 'serve.test_security']),
        ('upstream-ds41', [sys.executable, str(api.with_name('test_serve_ds41.py')), str(ROOT/'tokenizer')]),
        ('exact-memo', [sys.executable, str(ROOT/'test_exact_token_cache.py')]),
        ('strict-native-json', [sys.executable, str(ROOT/'test_strict_native_json.py'), str(ROOT/'source')]),
        ('combined-adapter', [sys.executable, str(ROOT/'test_combined_adapter.py')]),
        ('hybrid-native-json', [sys.executable, str(ROOT/'test_hybrid_native_json.py'), str(ROOT/'source')]),
        ('actual-api-canonical1m', [sys.executable, str(ROOT/'canonical-probe.py')])]
    for mode in ['0', '1']:
        env['ASTER_XQ_LITERAL_SCAN'] = mode
        for name, args in tests:
            prefix = 'cpu-' + mode + '-' + name
            with (ROOT/(prefix+'.stdout.log')).open('xb') as stdout, (ROOT/(prefix+'.stderr.log')).open('xb') as stderr:
                result = subprocess.run(args, cwd=ROOT/'source', env=env, stdout=stdout, stderr=stderr, timeout=60)
            STATE['tests'].append(dict(mode=mode, name=name, exit_code=result.returncode))
            save()
            if result.returncode:
                raise RuntimeError(prefix + ' failed')
    STATE.update(phase='passed', tests_completed=len(STATE['tests']), api_sha256=overlay['patched_sha256'], finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__+': '+str(error)[:2000], finished_unix=time.time())
    save()
    raise
