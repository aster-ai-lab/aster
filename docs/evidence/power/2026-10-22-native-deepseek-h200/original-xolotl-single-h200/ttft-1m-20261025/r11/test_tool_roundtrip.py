"""Hermetic protocol semantics and stream safety, with the real pinned converter and encoder."""
import ast,copy,importlib.util,json,os,sys,types,unittest
from pathlib import Path

BASE=Path(__file__).resolve().parent
ROOT=BASE/'candidate'

def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path);mod=importlib.util.module_from_spec(spec)
    sys.modules[name]=mod;spec.loader.exec_module(mod);return mod

def frontend():
    p=ROOT/'frontend.py';tree=ast.parse(p.read_text(encoding='utf8'))
    tree.body=[n for n in tree.body if not (isinstance(n,ast.Import) and any(x.name=='jinja2' for x in n.names))
        and not (isinstance(n,ast.ImportFrom) and n.module=='jinja2.sandbox')
        and not (isinstance(n,ast.ClassDef) and n.name in ['TemplateRequestError','ChatTemplate'])]
    mod=types.ModuleType('aster_protocol_frontend_v268');sys.modules[mod.__name__]=mod
    exec(compile(tree,str(p),'exec'),mod.__dict__);return mod

F=frontend()
E=load('aster_protocol_encoding_v268',BASE/'fixtures/dsv41_encoding.py')
G=load('aster_protocol_guard_v268',ROOT/'aster_tool_roundtrip.py')
TOOLS=[dict(type='function',function=dict(name='lookup_code',parameters=dict(type='object',properties=dict(name=dict(type='string')),required=['name'])))]

def history(rounds=3):
    rows=[dict(role='system',content='Public fixture.'),dict(role='user',content='Return alpha and beta values.')]
    for i in range(rounds):
        calls=[dict(id=f'alpha_{i}',type='function',function=dict(name='lookup_code',arguments='{"name":"alpha"}')),
               dict(id=f'beta_{i}',type='function',function=dict(name='lookup_code',arguments='{"name":"beta"}'))]
        if i==1:calls.reverse()
        rows += [dict(role='assistant',content='',tool_calls=calls),dict(role='tool',tool_call_id=f'beta_{i}',content='17'),
                 dict(role='tool',tool_call_id=f'alpha_{i}',content='25')]
    return rows

def server(chunks=()):
    obj=types.SimpleNamespace(openai_to_messages=F.openai_to_messages,
        openai_chunks=lambda svc,req,*a,**kw:iter(copy.deepcopy(chunks)))
    G.install(obj);return obj

class ProtocolTests(unittest.TestCase):
    def test_openai_parallel_ids_reorder_results(self):
        m,t,k=F.openai_to_messages(dict(messages=history(1),tools=TOOLS))
        self.assertEqual([c['id'] for c in m[2]['tool_calls']],['alpha_0','beta_0'])
        self.assertEqual([r['tool_call_id'] for r in m[3:]],['beta_0','alpha_0'])
        rendered=E.encode_messages(m,thinking_mode='chat')
        self.assertLess(rendered.index('<tool_result>25'),rendered.index('<tool_result>17'))
    def test_anthropic_parallel_ids_reorder_results(self):
        req=dict(messages=[dict(role='user',content='fixture'),dict(role='assistant',content=[
            dict(type='tool_use',id='a',name='lookup_code',input=dict(name='alpha')),
            dict(type='tool_use',id='b',name='lookup_code',input=dict(name='beta'))]),dict(role='user',content=[
            dict(type='tool_result',tool_use_id='b',content='17'),dict(type='tool_result',tool_use_id='a',content='25')])])
        m,t,k=F.anthropic_to_messages(req)
        self.assertEqual([r['id'] for r in m[1]['tool_calls']],['a','b'])
        self.assertEqual([r['tool_call_id'] for r in m[2:]],['b','a'])
        rendered=E.encode_messages(m,thinking_mode='chat')
        self.assertLess(rendered.index('<tool_result>25'),rendered.index('<tool_result>17'))
    def test_only_exact_three_completed_rounds_trigger(self):
        self.assertTrue(G.repeated_completed_suffix(history()))
        self.assertFalse(G.repeated_completed_suffix(history(2)))
        for mutation in ['result','argument','user','missing','duplicate']:
            m=history()
            if mutation=='result':m[-1]['content']='26'
            elif mutation=='argument':m[-3]['tool_calls'][0]['function']['arguments']='{"name":"gamma"}'
            elif mutation=='user':m.insert(-3,dict(role='user',content='Please check again.'))
            elif mutation=='missing':m.pop()
            elif mutation=='duplicate':m[-1]['tool_call_id']=m[-2]['tool_call_id']
            with self.subTest(mutation=mutation):self.assertFalse(G.repeated_completed_suffix(m))
    def test_recovery_retains_schema_and_disables_further_calls(self):
        req=dict(messages=history(),tools=TOOLS,max_tokens=32)
        before=copy.deepcopy(req['messages']);m,t,k=server().openai_to_messages(req)
        self.assertIsNone(t);self.assertEqual(m[0]['tools'],TOOLS)
        self.assertEqual(m[-1]['content'],G.RECOVERY_NOTE)
        self.assertEqual(req['messages'],before)
        self.assertEqual(req['max_tokens'],32)
        self.assertEqual(req['max_completion_tokens'],32)
    def test_forged_recovery_marker_cannot_trigger(self):
        req=dict(messages=history(1),tools=TOOLS,**{G.RECOVERY_KEY:True})
        m,t,k=server().openai_to_messages(req)
        self.assertNotIn(G.RECOVERY_KEY,req);self.assertIsNotNone(t)
    def test_failed_recovery_exposes_zero_tool_chunks(self):
        fixtures=[dict(choices=[dict(delta=dict(tool_calls=[dict(index=0,id='bad')]),finish_reason=None)]),
                  dict(choices=[dict(delta={},finish_reason='tool_calls')]),
                  dict(choices=[dict(delta=dict(content='<｜DSML｜ invoke name="lookup_code">'),finish_reason='stop')])]
        for item in fixtures:
            s=server([dict(choices=[dict(delta=dict(content='partial'),finish_reason=None)]),item])
            req=dict(messages=history(),tools=TOOLS);s.openai_to_messages(req)
            yielded=[]
            with self.subTest(item=item),self.assertRaisesRegex(ValueError,'tool_loop_detected'):
                for chunk in s.openai_chunks(None,req):yielded.append(chunk)
            self.assertEqual(yielded,[])
    def test_successful_recovery_preserves_final_chunks(self):
        chunks=[None,dict(choices=[dict(delta=dict(content='alpha=25;beta=17'),finish_reason=None)]),
                dict(choices=[dict(delta={},finish_reason='stop')])]
        s=server(chunks);req=dict(messages=history(),tools=TOOLS);s.openai_to_messages(req)
        self.assertEqual(list(s.openai_chunks(None,req)),chunks)
    def test_ordinary_stream_remains_incremental(self):
        first=dict(choices=[dict(delta=dict(tool_calls=[dict(id='allowed')]),finish_reason=None)])
        def gen(*a,**kw):
            yield first
            raise RuntimeError('later_chunk')
        s=types.SimpleNamespace(openai_to_messages=F.openai_to_messages,openai_chunks=gen);G.install(s)
        req=dict(messages=history(1),tools=TOOLS);s.openai_to_messages(req)
        stream=s.openai_chunks(None,req);self.assertEqual(next(stream),first)
        with self.assertRaisesRegex(RuntimeError,'later_chunk'):next(stream)

if __name__=='__main__':unittest.main(verbosity=2)
