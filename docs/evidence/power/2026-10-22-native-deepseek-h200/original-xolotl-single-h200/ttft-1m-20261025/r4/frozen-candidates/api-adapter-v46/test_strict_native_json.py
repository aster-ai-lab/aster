import json,sys,unittest
from pathlib import Path
sys.path.insert(0,str(Path(sys.argv[1])/'tools/v41/serve'))
import serve_ds41 as D
class StrictNativeArguments(unittest.TestCase):
 def parse(self,s):return D.ds41_parse_tool_call(' name="add_numbers">'+s).arguments
 def test_integers(self):self.assertEqual(self.parse('{"a":17,"b":25}'),{'a':17,'b':25})
 def test_nested_unicode(self):self.assertEqual(self.parse('{"a":[1,true,null,{"x":"中文"}]}'),{'a':[1,True,None,{'x':'中文'}]})
 def test_empty(self):self.assertEqual(self.parse('{}'),{});self.assertEqual(self.parse(''),{})
 def test_reject_unsupported_or_corrupt(self):
  for raw in ['[]','[1]','{"a":1} trailing','{"a":1,"a":2}','{"a":{"b":1,"b":2}}','{"a":NaN}','{"a":Infinity}','{"a":1e9999}','{"a":','null','undefined','<bad>17</bad>']:
   with self.subTest(raw=raw),self.assertRaises(ValueError):self.parse(raw)
 def test_canonical_dsml(self):
  body=D.P_OPEN+' name="a" string="false">17'+D.P_CLOSE+D.P_OPEN+' name="b" string="false">25'+D.P_CLOSE
  self.assertEqual(self.parse(body),{'a':17,'b':25})
 def test_canonical_string_and_invalid_tail(self):
  body=D.P_OPEN+' name="x" string="true">hello'+D.P_CLOSE
  self.assertEqual(self.parse(body),{'x':'hello'})
  with self.assertRaises(ValueError):self.parse(body+'garbage')
  with self.assertRaises(ValueError):self.parse(body+body)
sys.argv=[sys.argv[0]];unittest.main()
