"""Accept the observed complete-JSON plus single native parameter terminator grammar."""
import ast
import difflib
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
D = B / 'api-adapter-v52'
if D.exists():
    raise RuntimeError('V52 adapter is already frozen')
original = (B / 'api-adapter-v46/serve_ds41.py').read_bytes()
if hashlib.sha256(original).hexdigest() != 'f333c6929562009bab3096dd5bd22ddb08d764d1626593237a62fddf2315abee':
    raise RuntimeError('exact qualified previous adapter differs')
probe = json.loads((B / 'xolotl-tool-raw-probe-v51-latest.json').read_text())
lines = probe['raw_model_text_lines']
if not probe['public_fixture_only'] or len(lines) != 1 or not lines[0].startswith('[strata] raw: '):
    raise RuntimeError('exact observed public native-output fixture differs')
raw = ast.literal_eval(lines[0][len('[strata] raw: '):])
if type(raw) is not str or len(raw) > 8192:
    raise RuntimeError('bounded observed native text differs')
s = original.decode()
anchor = '''    if raw.startswith(("{", "[")):
        args = load(raw)
        if not isinstance(args, dict):'''
replacement = '''    if raw.startswith(("{", "[")):
        decoder = json.JSONDecoder(object_pairs_hook=pairs, parse_constant=reject_constant, parse_float=finite_float)
        args, end = decoder.raw_decode(raw)
        # The native model also emits a complete JSON object followed by one
        # DSML parameter terminator. The JSON is decoded intact; only this exact
        # outer suffix is accepted. Strings, keys and values are never repaired.
        if raw[end:].strip() not in ("", P_CLOSE):
            raise ValueError("unsupported native JSON argument terminator")
        if not isinstance(args, dict):'''
if s.count(anchor) != 1:
    raise RuntimeError('exact strict JSON argument branch differs')
s = s.replace(anchor, replacement)
compile(s, 'hybrid-native-json-v52', 'exec')
D.mkdir()
for name in ['aster_token_cache.py', 'test_exact_token_cache.py', 'test_strict_native_json.py',
             'test_combined_adapter.py', 'benchmark_real_tokenizer.py']:
    (D / name).write_bytes((B / 'api-adapter-v46' / name).read_bytes())
(D / 'serve_ds41.original.py').write_bytes(original)
(D / 'serve_ds41.py').write_text(s, encoding='utf-8', newline='\n')
fixture = dict(public_fixture_only=True, source='xolotl-tool-raw-probe-v51-latest.json',
    raw_generated_text=raw, forced_prefix='<｜DSML｜ invoke name="add_numbers">',
    expected_name='add_numbers', expected_arguments=dict(a=17, b=25),
    raw_sha256=hashlib.sha256(raw.encode()).hexdigest(), actual_model_binary_sha256='34f3c963304445e50c4ae398194f4b4745a0b1868a6b4a1d6668068aebcf96e6')
(D / 'actual-tool-output.json').write_text(json.dumps(fixture, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
test = r'''import json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(Path(sys.argv[1])/'tools/v41/serve'))
import serve_ds41 as D

class NativeHybridArguments(unittest.TestCase):
 def parse(self,text):return D.ds41_parse_tool_call(' name="add_numbers">'+text).arguments
 def test_observed_complete_native_frame(self):
  fixture=json.loads((ROOT/'actual-tool-output.json').read_text())
  raw=fixture['raw_generated_text']
  end=D.ds41_call_end(raw)
  self.assertGreaterEqual(end,0)
  self.assertEqual(self.parse(raw[:end]),fixture['expected_arguments'])
 def test_exact_optional_single_terminator(self):
  for suffix in ['',D.P_CLOSE,'\n'+D.P_CLOSE+'\n']:
   self.assertEqual(self.parse('{"a":17,"b":25}'+suffix),{'a':17,'b':25})
 def test_closing_tag_inside_json_string_is_data(self):
  value={'a':D.P_CLOSE,'b':{'x':'before '+D.P_CLOSE+' after'}}
  for suffix in ['',D.P_CLOSE]:self.assertEqual(self.parse(json.dumps(value)+suffix),value)
 def test_unknown_or_multiple_suffixes_refused(self):
  for suffix in ['junk',D.P_CLOSE+D.P_CLOSE,D.P_CLOSE+'junk',D.P_CLOSE+'{}','</unknown>','{}']:
   with self.subTest(suffix=suffix),self.assertRaises(ValueError):self.parse('{"a":17,"b":25}'+suffix)
 def test_invalid_objects_still_refused(self):
  for text in ['[]','[1]','{"a":1,"a":2}','{"a":NaN}','{"a":Infinity}','{"a":1e9999}','{"a":','null']:
   with self.subTest(text=text),self.assertRaises(ValueError):self.parse(text+D.P_CLOSE)
 def test_literal_parameter_grammar_unchanged(self):
  raw=D.P_OPEN+' name="a" string="false">17'+D.P_CLOSE+D.P_OPEN+' name="b" string="false">25'+D.P_CLOSE
  self.assertEqual(self.parse(raw),{'a':17,'b':25})

sys.argv=[sys.argv[0]];unittest.main()
'''
compile(test, 'test-native-hybrid-v52', 'exec')
(D / 'test_hybrid_native_json.py').write_text(test, encoding='utf-8', newline='\n')
(D / 'hybrid-native-json-v52.patch').write_text(''.join(difflib.unified_diff(original.decode().splitlines(keepends=True), s.splitlines(keepends=True),
    fromfile='v46/serve_ds41.py', tofile='v52/serve_ds41.py')), encoding='utf-8', newline='\n')
meta = dict(upstream_sha256=hashlib.sha256(original).hexdigest(), patched_sha256=hashlib.sha256(s.encode()).hexdigest(),
    helper_sha256=hashlib.sha256((D / 'aster_token_cache.py').read_bytes()).hexdigest(),
    accepted_outer_suffixes=['empty', 'exactly one DSML parameter terminator'],
    arbitrary_suffixes_accepted=False, json_values_repaired=False, gpu_qualified=False, production_changed=False)
(D / 'overlay-plan.json').write_text(json.dumps(meta, indent=2) + '\n')
print(json.dumps(dict(phase='prepared', **meta)))
