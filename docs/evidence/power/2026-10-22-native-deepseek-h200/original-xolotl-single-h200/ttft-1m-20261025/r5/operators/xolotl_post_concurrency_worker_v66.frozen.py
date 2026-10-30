"""One exact single-request replay after the failed concurrent block; retain the original failure."""
import ast
import hashlib
import json
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
plan = json.loads((ROOT / 'request-plan.json').read_text())
source = Path(plan['source_worker'])
raw = source.read_bytes()
if source.is_symlink() or hashlib.sha256(raw).hexdigest() != plan['source_worker_sha256']:
    raise RuntimeError('exact original full-window request worker differs')
tree = ast.parse(raw)
if sum(isinstance(node, ast.Try) for node in tree.body) != 1:
    raise RuntimeError('exact original request-only module boundary differs')
module = ast.Module(body=[node for node in tree.body if not isinstance(node, ast.Try)], type_ignores=[])
namespace = dict(__file__=str(ROOT / 'request-only.py'), __name__='frozen_request_only')
exec(compile(module, str(source), 'exec'), namespace)
state = namespace['STATE']
save = namespace['save']
try:
    original = json.loads(Path(plan['reference_result']).read_text())
    if (original['phase'] != 'failed' or original['error'] != 'ValueError: full-window long-output identity differs'
            or not original['full1m_cache_restore_passed'] or len(original['tests']) != 20):
        raise RuntimeError('exact completed original concurrency failure differs')
    fixture = Path(plan['fixture_root'])
    metadata = json.loads((fixture / 'fixture-metadata.json').read_text())
    text = (fixture / 'public-fixture.txt').read_text()
    if hashlib.sha256(text.encode()).hexdigest() != metadata['prompt_sha256']:
        raise RuntimeError('same exact full-window public prompt differs')
    state.update(schema='aster-xolotl-post-concurrency-single-v66', phase='single_replay',
                 original_suite_failure_retained=True)
    save()
    row = namespace['ask']('post-concurrent-single-1024', text)
    namespace['check_completion'](row, metadata, cold=False, reference_sha256=original['cold_output_sha256'])
    state.update(phase='passed', post_concurrent_single_matches_cold=True, finished_unix=time.time())
    save()
except BaseException as error:
    state.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
