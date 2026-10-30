"""Compare exact IDs and CPU encode times with the deployed model tokenizer."""
from array import array
import hashlib, json, statistics, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = json.loads((ROOT / 'benchmark-plan.json').read_text())
sys.path.insert(0, str(ROOT / 'source/tools/v41/serve'))
import serve_ds41 as D

vocab = json.loads((ROOT / 'tokenizer/vocab.json').read_text())
tokens = [None] * len(vocab)
for word, index in vocab.items():
    tokens[index] = word
tk = D.Ds41Tokenizer(tokens, (ROOT / 'tokenizer/merges.txt').read_text().split('\n'),
                     json.loads((ROOT / 'tokenizer/token_type.json').read_text()))
fixture = Path(P['public_fixture']).read_bytes()
if hashlib.sha256(fixture).hexdigest() != P['fixture_sha256']:
    raise RuntimeError('qualified full-window public fixture differs')
text = D.Ds41Template().render([dict(role='user', content=fixture.decode())], enable_thinking=False)

def digest(ids):
    return hashlib.sha256(array('I', ids).tobytes()).hexdigest()

baseline = []
for _ in range(3):
    start = time.perf_counter()
    ids = D._Tok.encode(tk, text, parse_special=True)
    baseline.append(time.perf_counter() - start)
    if len(ids) != 1048504 or digest(ids) != P['expected_id_sha256']:
        raise RuntimeError('original full-window tokenizer ID identity differs')
start = time.perf_counter()
ids = tk.encode(text, parse_special=True)
fill = time.perf_counter() - start
if digest(ids) != P['expected_id_sha256']:
    raise RuntimeError('memo miss changed exact full-window token IDs')
hot = []
for _ in range(5):
    start = time.perf_counter()
    ids = tk.encode(text, parse_special=True)
    hot.append(time.perf_counter() - start)
    if digest(ids) != P['expected_id_sha256']:
        raise RuntimeError('memo hit changed exact full-window token IDs')
ids[0] = -1
if digest(tk.encode(text, parse_special=True)) != P['expected_id_sha256']:
    raise RuntimeError('caller mutation changed cached token IDs')
prefix = ' cat' * 2048
literal = D.E.eos_token
cases = [prefix + literal + suffix for suffix in [' 中文 é e\u0301 😀', ' different', ' different ']]
checks = 0
for sample in cases:
    for mode in [False, True]:
        for plain in [(), ((len(prefix), len(prefix) + len(literal)),)]:
            wanted = D._Tok.encode(tk, sample, parse_special=mode, plain=plain)
            for _ in range(2):
                if tk.encode(sample, parse_special=mode, plain=plain) != wanted:
                    raise RuntimeError('memo changed Unicode/special-mode/literal-span token IDs')
                checks += 1
stats = tk._aster_encode_memo.stats()
if stats['accounted_bytes'] > 32 * 1024**2 or stats['entries'] > 4:
    raise RuntimeError('bounded token memo exceeded its declared capacity')
result = dict(unix=time.time(), phase='passed', gpu_executed=False, actual_prompt_tokens=1048504,
              id_sha256=P['expected_id_sha256'], baseline_seconds=baseline,
              baseline_median_seconds=statistics.median(baseline), memo_fill_seconds=fill,
              memo_hit_seconds=hot, memo_hit_median_seconds=statistics.median(hot),
              literal_unicode_mode_comparisons=checks, memo_stats=stats,
              scope='CPU encode only, identical full rendered prompt; no prefix reuse or GPU/end-to-end gain claimed')
(ROOT / 'real-tokenizer-benchmark.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
