"""Replay the complete real API encoder against canonical IDs with either startup flag."""
from array import array
import hashlib
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'literal-preflight-plan.json').read_text())
sys.path[:0] = [str(ROOT / 'source/tools/v41/serve')]
import serve_ds41 as D

mode = os.environ['ASTER_XQ_LITERAL_SCAN']
if mode not in ['0', '1'] or (D._aster_literal_scan is not None) != (mode == '1'):
    raise RuntimeError('exact requested literal scanner installation differs')
vocab = json.loads((ROOT / 'tokenizer/vocab.json').read_text())
tokens = [None] * len(vocab)
for word, index in vocab.items():
    tokens[index] = word
tk = D.Ds41Tokenizer(tokens, (ROOT / 'tokenizer/merges.txt').read_text().split('\n'), json.loads((ROOT / 'tokenizer/token_type.json').read_text()))
svc = object.__new__(D.S.Service)
svc.tok = tk
svc.template = D.Ds41Template()
svc.effort_end = False
svc.literals = D.F.literal_tags(tk.control_tokens)
path = Path(PLAN['fixture_root']) / 'public-fixture.txt'
metadata = json.loads((path.parent / 'fixture-metadata.json').read_text())
if path.is_symlink() or path.stat().st_size != metadata['prompt_bytes'] or path.stat().st_size > 8 * 1024**2:
    raise RuntimeError('bounded exact canonical public body differs')
raw = path.read_bytes()
if hashlib.sha256(raw).hexdigest() != metadata['prompt_sha256']:
    raise RuntimeError('exact canonical public body SHA differs')
begin = time.monotonic()
ids = svc.encode_prompt([dict(role='user', content=raw.decode())], None, dict(enable_thinking=False))
elapsed = time.monotonic() - begin
packed = array('I', ids)
if packed.itemsize != 4:
    raise RuntimeError('ID digest requires four-byte unsigned ints')
if sys.byteorder != 'little':
    packed.byteswap()
sha = hashlib.sha256(packed.tobytes()).hexdigest()
if len(ids) != 1047544 or sha != metadata['prompt_id_sha256']:
    raise RuntimeError('real patched API canonical IDs differ')
print(json.dumps(dict(phase='passed', mode=mode, actual_service_encode_seconds=elapsed,
    prompt_tokens=len(ids), prompt_id_sha256=sha, memo=tk._aster_encode_memo.stats(),
    scanner=D._aster_literal_scan.stats() if D._aster_literal_scan is not None else None,
    gpu_executed=False, model_requests_sent=0)))
