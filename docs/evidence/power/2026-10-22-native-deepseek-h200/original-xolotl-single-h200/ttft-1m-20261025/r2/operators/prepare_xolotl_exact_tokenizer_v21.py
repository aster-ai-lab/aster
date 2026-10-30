"""Patch only the exact original tokenizer adapter, preserving its parsing and IDs."""
import ast, difflib, hashlib, json
from pathlib import Path

root = Path(__file__).resolve().parent / 'tokenizer-v21'
raw = (root / 'serve_ds41.original.py').read_bytes()
original = json.loads((root / 'original-adapter.json').read_text())
if hashlib.sha256(raw).hexdigest() != original['source_sha256']:
    raise RuntimeError('actual original tokenizer adapter differs')
source = raw.decode()

def replace_once(before, after):
    global source
    if source.count(before) != 1:
        raise RuntimeError('exact tokenizer overlay anchor differs: ' + before[:100])
    source = source.replace(before, after, 1)

replace_once('from pathlib import Path\n', 'from pathlib import Path\nfrom aster_token_cache import ExactEncodeCache\n')
replace_once('        self._literal = {i: t.encode("utf-8") for t, i in self.special_tokens.items()}\n',
             '        self._literal = {i: t.encode("utf-8") for t, i in self.special_tokens.items()}\n'
             '        self._aster_encode_memo = ExactEncodeCache(super().encode)\n')
replace_once('    def token_bytes(self, i: int) -> bytes:\n', '''    def encode(self, text: str, parse_special: bool = False, plain=()) -> list[int]:
        memo = getattr(self, "_aster_encode_memo", None)
        if memo is None:
            return super().encode(text, parse_special=parse_special, plain=plain)
        return memo.encode(text, parse_special=parse_special, plain=plain)

    def token_bytes(self, i: int) -> bytes:
''')
ast.parse(source)
patched = root / 'serve_ds41.py'
with patched.open('x', encoding='utf-8', newline='\n') as stream:
    stream.write(source)
patch = ''.join(difflib.unified_diff(raw.decode().splitlines(True), source.splitlines(True),
                                     fromfile='a/tools/v41/serve/serve_ds41.py', tofile='b/tools/v41/serve/serve_ds41.py'))
(root / 'exact-tokenizer-v21.patch').write_text(patch, encoding='utf-8', newline='\n')
record = dict(upstream_sha256=original['source_sha256'], patched_sha256=hashlib.sha256(patched.read_bytes()).hexdigest(),
              helper_sha256=hashlib.sha256((root / 'aster_token_cache.py').read_bytes()).hexdigest(),
              gpu_qualified=False, production_changed=False, max_memo_bytes=32 * 1024**2, max_memo_entries=4)
(root / 'overlay-plan.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record))
