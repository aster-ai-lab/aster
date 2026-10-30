"""Finite, CPU-only A/B/A of the real pinned API encoder, with exact ID checks."""
from array import array
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', runs=[], gpu_executed=False,
             model_requests_sent=0, live_service_changed=False, input_text_retained=False)

def save():
    p = ROOT / 'cpu-result.tmp'
    p.write_text(json.dumps(STATE, indent=2) + '\n')
    p.replace(ROOT / 'cpu-result.json')

def digest(ids):
    data = array('I', ids)
    if data.itemsize != 4:
        raise RuntimeError('exact little-endian uint32 ID digest requires four-byte unsigned ints')
    if sys.byteorder != 'little':
        data.byteswap()
    return hashlib.sha256(data.tobytes()).hexdigest()

def deadline(signum, frame):
    raise TimeoutError('finite 160-second CPU encoder replay exceeded')

try:
    if os.getuid() != 10250 or ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250:
        raise RuntimeError('exact CPU replay root/owner differs')
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    os.environ['ASTER_XQ_EXACT_TOKEN_CACHE'] = '1'
    os.environ['ASTER_XQ_LITERAL_SCAN'] = '1'
    os.nice(19)
    signal.signal(signal.SIGALRM, deadline)
    signal.alarm(160)
    save()
    import test_literal_scan
    log = io.StringIO()
    tests = unittest.TextTestRunner(stream=log, verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(test_literal_scan))
    (ROOT / 'unit-tests.log').write_text(log.getvalue())
    STATE['unit_methods_passed'] = tests.testsRun
    if not tests.wasSuccessful():
        raise RuntimeError('literal scanner hermetic acceptance failed')
    source = Path(PLAN['source_public_root'])
    fixture = Path(PLAN['fixture_root'])
    for path in [source, fixture]:
        if path.resolve() != path or path.stat().st_uid != 10250:
            raise RuntimeError('exact public source/fixture root differs')
    api = source / 'source/tools/v41/serve/serve_ds41.py'
    if api.is_symlink() or hashlib.sha256(api.read_bytes()).hexdigest() != PLAN['api_sha256']:
        raise RuntimeError('exact pinned API source differs')
    sys.path[:0] = [str(source / 'python-deps-preflight'), str(source / 'source'), str(source / 'source/tools/v41/serve')]
    import serve_ds41 as D
    import aster_literal_scan as L
    frontend_sha = hashlib.sha256(Path(D.F.__file__).read_bytes()).hexdigest()
    if frontend_sha != L.FRONTEND_SHA256:
        raise RuntimeError('pinned frontend SHA differs')
    vocab = json.loads((source / 'tokenizer/vocab.json').read_text())
    tokens = [None] * len(vocab)
    for word, index in vocab.items():
        tokens[index] = word
    tk = D.Ds41Tokenizer(tokens, (source / 'tokenizer/merges.txt').read_text().split('\n'),
                         json.loads((source / 'tokenizer/token_type.json').read_text()))
    metadata = json.loads((fixture / 'fixture-metadata.json').read_text())
    p = fixture / 'public-fixture.txt'
    if p.is_symlink() or p.stat().st_size != metadata['prompt_bytes'] or p.stat().st_size > 8 * 1024**2:
        raise RuntimeError('bounded public fixture bytes differ')
    raw = p.read_bytes()
    if hashlib.sha256(raw).hexdigest() != metadata['prompt_sha256']:
        raise RuntimeError('exact public fixture SHA differs')
    messages = [dict(role='user', content=raw.decode('utf-8', 'strict'))]
    kwargs = dict(enable_thinking=False)
    svc = object.__new__(D.S.Service)
    svc.tok = tk
    svc.template = D.Ds41Template()
    svc.effort_end = False
    svc.literals = D.F.literal_tags(tk.control_tokens)
    original_has, original_mark = D.F._has_tag, D.F._mark
    STATE.update(frontend_sha256=frontend_sha, api_sha256=PLAN['api_sha256'], literal_map_entries=len(svc.literals),
                 original_metadata_prompt_id_sha256=metadata['prompt_id_sha256'], prompt_sha256=metadata['prompt_sha256'])
    reference = None
    canonical_rendered = canonical_plain = None
    cache = None
    for mode in ['control_a', 'literal_scan', 'control_b']:
        if mode == 'literal_scan':
            cache = L.install(D.F)
            if cache is None or not cache.prime(svc.literals):
                raise RuntimeError('actual authoritative literal map is ineligible for the exact fast path')
            if D.F._has_tag('<think>') != original_has('<think>') or D.F._mark('<think>') != original_mark('<think>'):
                raise RuntimeError('frontend default-argument behavior differs')
        elif mode == 'control_b':
            D.F._has_tag, D.F._mark = original_has, original_mark
        STATE.update(phase='encoding', current=mode)
        save()
        begin = time.monotonic()
        marked, tools, changed = D.F.mark_think_literals(messages, None, svc.literals)
        marked_at = time.monotonic()
        rendered = svc.render_prompt(marked, tools, kwargs)
        rendered_at = time.monotonic()
        if changed:
            rendered, plain = D.F.unmark_think_literals(rendered, svc.literals)
        else:
            plain = ()
        unmarked_at = time.monotonic()
        ids = tk.encode(rendered, parse_special=True, plain=plain)
        encoded_at = time.monotonic()
        staged_sha = digest(ids)
        if reference is None:
            reference = ids
            canonical_rendered, canonical_plain = rendered, plain
        if ids != reference or staged_sha != PLAN['canonical_api_id_sha256'] or len(ids) != metadata['actual_prompt_tokens']:
            raise RuntimeError('exact staged API encoder IDs differ')
        begin_api = time.monotonic()
        actual = svc.encode_prompt(messages, None, kwargs)
        api_elapsed = time.monotonic() - begin_api
        if actual != reference:
            raise RuntimeError('actual Service.encode_prompt differs from staged encoder')
        STATE['runs'].append(dict(mode=mode, mark_seconds=marked_at-begin, render_seconds=rendered_at-marked_at,
            unmark_seconds=unmarked_at-rendered_at, tokenize_seconds=encoded_at-unmarked_at,
            staged_seconds=encoded_at-begin, actual_service_encode_seconds=api_elapsed, prompt_tokens=len(actual),
            prompt_id_sha256=staged_sha, literal_spans=len(plain), changed=changed,
            memo=tk._aster_encode_memo.stats(), scanner=cache.stats() if cache is not None else None))
        save()
    direct_rendered = svc.render_prompt(messages, None, kwargs)
    direct = tk.encode(direct_rendered, parse_special=True)
    if digest(direct) != metadata['prompt_id_sha256']:
        raise RuntimeError('original direct-template fixture digest was not reproduced')
    positions = {}
    for name, marker in [('begin', 'BEGIN_RECORD_CODE:'), ('middle', 'MIDDLE_RECORD_CODE:'), ('end', 'END_RECORD_CODE:')]:
        end = canonical_rendered.index(marker)
        spans = [(a, min(b, end)) for a, b in canonical_plain if a < end]
        positions[name] = len(tk.encode(canonical_rendered[:end], parse_special=True, plain=spans))
    if not (positions['begin'] < 128 and abs(positions['middle'] - len(reference)//2) < 256 and positions['end'] > len(reference)-512):
        raise RuntimeError('canonical API fixture record placement differs')
    corrected = dict(metadata)
    corrected.update(schema='aster-public-code-fullwindow-canonical-api-v143',
        prompt_id_sha256=digest(reference), record_token_positions=positions,
        previous_direct_template_prompt_id_sha256=metadata['prompt_id_sha256'],
        canonical_encoder='Service.encode_prompt: mark_think_literals/render_prompt/unmark_think_literals/tokenizer.encode',
        canonical_api_sha256=PLAN['api_sha256'], canonical_frontend_sha256=frontend_sha,
        body_changed=False, input_tokens_changed=False, protection_rules_changed=False,
        native_runtime_id_digest_captured=False)
    (ROOT / 'fixture-metadata-canonical.json').write_text(json.dumps(corrected, indent=2) + '\n')
    differences = [i for i, (a, b) in enumerate(zip(reference, direct)) if a != b]
    STATE.update(phase='passed', current=None, all_A_B_A_ids_exact=True,
        canonical_prompt_id_sha256=digest(reference), direct_prompt_id_sha256=digest(direct),
        direct_and_protected_rendered_text_equal=direct_rendered == canonical_rendered,
        direct_and_protected_token_differences=len(differences), first_different_token_offsets=differences[:8],
        canonical_record_token_positions=positions, end_to_end_gpu_speed_claimed=False,
        actual_model_speed_claimed=False, finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error)[:2000], finished_unix=time.time())
    save()
    raise
finally:
    signal.alarm(0)
