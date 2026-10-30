"""Compile a GPU gate without executing it or initializing CUDA."""
import hashlib, json, os, subprocess, time
from pathlib import Path

root = Path(__file__).resolve().parent
plan = json.loads((root / 'compile-plan.json').read_text())
state = dict(unix=time.time(), phase='preflight', gpu_executed=False)

def save():
    temp = root / 'compile-result.tmp'
    temp.write_text(json.dumps(state, indent=2) + '\n')
    temp.replace(root / 'compile-result.json')

try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    save()
    if root.resolve() != root or root.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('GPU gate compile owner differs')
    for row in plan['sources']:
        path = root / row['file']
        if path.is_symlink() or path.stat().st_size != row['bytes'] or hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
            raise RuntimeError('GPU gate frozen source differs')
    state['phase'] = 'compiling'
    save()
    with (root / 'compile.log').open('xb') as log:
        result = subprocess.run(plan['argv'], cwd=root, stdout=log, stderr=log, timeout=45)
    if result.returncode:
        raise RuntimeError('GPU gate compile exit ' + str(result.returncode))
    binary = root / 'mixed_cache_gpu_test'
    state.update(phase='built', binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), binary_bytes=binary.stat().st_size)
except BaseException as error:
    state.update(phase='failed', error=type(error).__name__ + ': ' + str(error))
    raise
finally:
    state['finished_unix'] = time.time()
    save()
