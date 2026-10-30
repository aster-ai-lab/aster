"""Combine pinned strict DSML parsing with independently switchable exact token memo."""
import ast
import difflib
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
D = B / 'api-adapter-v46'
if D.exists():
    raise RuntimeError('V46 adapter already frozen; retain its inputs')
D.mkdir()
source = (B / 'tokenizer-v21/serve_ds41.original.py').read_bytes()
if hashlib.sha256(source).hexdigest() != '5180c184d1a421dd5b56e799360fe6306b96d41da2d443d0bb68dae6714f2e5d':
    raise RuntimeError('pinned original adapter differs')
s = (B / 'tokenizer-v21/serve_ds41.py').read_text()
strict = (B / 'serve_ds41_dsml_json_v6b.frozen.py').read_text()


def function(text, name):
    nodes = [n for n in ast.parse(text).body if isinstance(n, ast.FunctionDef) and n.name == name]
    if len(nodes) != 1:
        raise RuntimeError('exact adapter function differs: ' + name)
    node = nodes[0]
    return node, ''.join(text.splitlines(keepends=True)[node.lineno - 1:node.end_lineno])


node, _ = function(s, 'ds41_parse_tool_call')
_, body = function(strict, 'ds41_parse_tool_call')
lines = s.splitlines(keepends=True)
s = ''.join(lines[:node.lineno - 1]) + body + '\n' + ''.join(lines[node.end_lineno:])
anchor = 'class Ds41Tokenizer(_Tok):'
if s.count(anchor) != 1:
    raise RuntimeError('exact tokenizer class differs')
s = s.replace(anchor, '''def _aster_exact_memo_enabled():
    mode = os.environ.get("ASTER_XQ_EXACT_TOKEN_CACHE", "0")
    if mode not in ("0", "1"):
        raise ValueError("ASTER_XQ_EXACT_TOKEN_CACHE must be 0 or 1")
    return mode == "1"


''' + anchor)
anchor = 'self._aster_encode_memo = ExactEncodeCache(super().encode)'
if s.count(anchor) != 1:
    raise RuntimeError('exact bounded memo construction differs')
s = s.replace(anchor, 'self._aster_encode_memo = ExactEncodeCache(super().encode) if _aster_exact_memo_enabled() else None')
compile(s, 'serve-ds41-adapter-v46', 'exec')
(D / 'serve_ds41.py').write_text(s, encoding='utf-8', newline='\n')
(D / 'serve_ds41.original.py').write_bytes(source)
for name in ['aster_token_cache.py', 'test_exact_token_cache.py', 'benchmark_real_tokenizer.py']:
    (D / name).write_bytes((B / 'tokenizer-v21' / name).read_bytes())
(D / 'test_strict_native_json.py').write_bytes((B / 'test_xolotl_dsml_json_v6b.frozen.py').read_bytes())
test = r'''"""Actual BPE stream boundaries and strict cache modes, no model or credential."""
import codecs
import json
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'source/tools/v41/serve'))
import serve_ds41 as D


class CombinedAdapterTests(unittest.TestCase):
    def test_closed_memo_mode(self):
        with patch.dict(os.environ, {}, clear=True):
            self.assertFalse(D._aster_exact_memo_enabled())
        for raw, expected in [('0', False), ('1', True)]:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': raw}):
                self.assertEqual(D._aster_exact_memo_enabled(), expected)
        for raw in ['', '2', 'true', ' 1', '1 ', '01', '-1']:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': raw}):
                with self.assertRaisesRegex(ValueError, 'must be 0 or 1'):
                    D._aster_exact_memo_enabled()

    def test_actual_bpe_streams_in_both_cache_modes(self):
        vocab = json.loads((ROOT / 'tokenizer/vocab.json').read_text())
        tokens = [None] * len(vocab)
        for word, i in vocab.items():
            tokens[i] = word
        tools = [dict(name='add_numbers', description='Add two integers.', parameters=dict(
            type='object', properties=dict(a=dict(type='integer'), b=dict(type='integer')), required=['a', 'b']))]
        arguments = dict(a=17, b=25)
        prefix = D.INVOKE_OPEN + ' name="add_numbers">'
        bodies = [json.dumps(arguments), D.P_OPEN + ' name="a" string="false">17' + D.P_CLOSE
                  + D.P_OPEN + ' name="b" string="false">25' + D.P_CLOSE]
        passed = 0
        for mode in ['0', '1']:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': mode}):
                tk = D.Ds41Tokenizer(tokens, (ROOT / 'tokenizer/merges.txt').read_text().split('\n'),
                                      json.loads((ROOT / 'tokenizer/token_type.json').read_text()))
            self.assertEqual(tk._aster_encode_memo is not None, mode == '1')
            for body in bodies:
                for wrap in [False, True]:
                    for forced in [False, True]:
                        for width in [1, 7, 1000, 'bpe']:
                            with self.subTest(mode=mode, body=body, wrap=wrap, forced=forced, width=width):
                                parser = D.F.OutputParser(thinking=False, tools=tools)
                                text = prefix + body + D.INVOKE_CLOSE
                                if wrap:
                                    text = D.CALLS_OPEN + text + D.CALLS_CLOSE
                                events = []
                                if forced:
                                    events += parser.feed(prefix)
                                    text = body + D.INVOKE_CLOSE
                                if width == 'bpe':
                                    decoder = codecs.getincrementaldecoder('utf-8')()
                                    chunks = [decoder.decode(tk.token_bytes(i)) for i in tk.encode(text, parse_special=True)]
                                    chunks.append(decoder.decode(b'', final=True))
                                else:
                                    chunks = [text[i:i + width] for i in range(0, len(text), width)]
                                for chunk in chunks:
                                    events += parser.feed(chunk)
                                events += parser.finish(reason='stop')
                                calls = [dict(name=e.call.name, arguments=e.call.arguments)
                                         for e in events if e.kind == 'tool_call']
                                self.assertEqual(calls, [dict(name='add_numbers', arguments=arguments)])
                                passed += 1
        self.assertEqual(passed, 64)
        print('PASS 64 complete native stream cases, including actual BPE token boundaries and both cache modes')


if __name__ == '__main__':
    unittest.main()
'''
compile(test, 'test-combined-adapter-v46', 'exec')
(D / 'test_combined_adapter.py').write_text(test, encoding='utf-8', newline='\n')
worker = (B / 'tokenizer-v21/cpu_preflight_worker.py').read_text()
anchor = "CUDA_VISIBLE_DEVICES='')"
if worker.count(anchor) != 1:
    raise RuntimeError('exact CPU preflight environment differs')
worker = worker.replace(anchor, "CUDA_VISIBLE_DEVICES='',ASTER_XQ_EXACT_TOKEN_CACHE='1')")
anchor = "('actual-fullwindow-tokenizer',[sys.executable,str(ROOT/'benchmark_real_tokenizer.py')])"
if worker.count(anchor) != 1:
    raise RuntimeError('exact CPU preflight test list differs')
worker = worker.replace(anchor, "('strict-native-json',[sys.executable,str(ROOT/'test_strict_native_json.py'),str(ROOT/'source')]),('combined-adapter',[sys.executable,str(ROOT/'test_combined_adapter.py')])," + anchor)
compile(worker, 'cpu-adapter-worker-v46', 'exec')
(D / 'cpu_preflight_worker.py').write_text(worker, encoding='utf-8', newline='\n')
helper = (D / 'aster_token_cache.py').read_bytes()
overlay = dict(upstream_sha256=hashlib.sha256(source).hexdigest(), patched_sha256=hashlib.sha256(s.encode()).hexdigest(),
               helper_sha256=hashlib.sha256(helper).hexdigest(), strict_grammar_basis='serve_ds41_dsml_json_v6b.frozen.py',
               memo_basis='tokenizer-v21', memo_env='ASTER_XQ_EXACT_TOKEN_CACHE', memo_default='0',
               max_memo_bytes=32 * 1024**2, max_memo_entries=4, gpu_qualified=False, production_changed=False)
(D / 'overlay-plan.json').write_text(json.dumps(overlay, indent=2) + '\n', encoding='utf-8', newline='\n')
(D / 'combined-api-v46.patch').write_text(''.join(difflib.unified_diff(source.decode().splitlines(keepends=True), s.splitlines(keepends=True),
    fromfile='a/tools/v41/serve/serve_ds41.py', tofile='b/tools/v41/serve/serve_ds41.py')), encoding='utf-8', newline='\n')
stage = (B / 'stage_xolotl_exact_tokenizer_v21.frozen.py').read_text()
replacements = {
    "xolotl-exact-tokenizer-v21-intent.json": "xolotl-api-adapter-v46-intent.json",
    "xolotl-stable-service-v15-intent.json": "xolotl-indexer-service-v44-intent.json",
    "xolotl-full-1m-qualification-v17-latest.json": "xolotl-resident-v38-fullwindow-latest.json",
    "['request-result.json']": "['result']",
    "BASE / 'tokenizer-v21'": "BASE / 'api-adapter-v46'",
    "'/tokenizer-preflight-v21'": "'/api-preflight-v46'",
    "stable['root'] + '/qualification-full-1m-v17/public-fixture.txt'": "json.loads((BASE / 'xolotl-resident-v38-fullwindow-intent.json').read_text())['root'] + '/public-fixture.txt'",
}
for old, new in replacements.items():
    if old not in stage:
        raise RuntimeError('exact public CPU staging anchor differs: ' + old)
    stage = stage.replace(old, new)
anchor = "    'cpu_preflight_worker.py': (local / 'cpu_preflight_worker.py').read_bytes(),"
if stage.count(anchor) != 1:
    raise RuntimeError('exact CPU payload list differs')
stage = stage.replace(anchor, "    'test_strict_native_json.py': (local / 'test_strict_native_json.py').read_bytes(),\n    'test_combined_adapter.py': (local / 'test_combined_adapter.py').read_bytes(),\n" + anchor)
compile(stage, 'stage-combined-api-v46', 'exec')
(B / 'stage_xolotl_api_adapter_v46.frozen.py').write_text(stage, encoding='utf-8', newline='\n')
print(json.dumps(dict(phase='prepared', **overlay)))
