import json,sys,unittest
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
