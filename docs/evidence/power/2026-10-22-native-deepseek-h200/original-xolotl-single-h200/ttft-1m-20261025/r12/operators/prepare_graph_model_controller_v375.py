"""Keep the full original model gates, with bounded read-only observer retries."""
import ast,hashlib,json
from xolotl_trial_common import BASE
s=(BASE/'run_topk_native_model_aba_v213.frozen.py').read_text()
s=s.replace('import json\n','import json\nimport os\nfrom aster_observer_retry_v236r1 import observe_with_retries,run_json\n',1)
s=s.replace("profiles=['control_a','topk','control_b']","profiles=['control_a','guard','control_b']").replace('topk-native','graph-native').replace('topk_native','graph_native').replace('topk-fullwindow','graph-fullwindow').replace('topk_fullwindow','graph_fullwindow').replace('v212','v373').replace('v213','v375').replace('v214','v374').replace("summaries['topk']","summaries['guard']")
s=s.replace('2bc83dcc3d3993ae7473b4e289454672f04190f21924e672de8dd53e2aff0195','b7124c78d711d3f48b928a292bd6423af4137816533e0acfbeb5114644508185')
s=s.replace("'1cfa236c-40ba-408f-a45e-003e0261bdf5'","'bd638591-0ba6-4c60-9eb6-129a531714ee'").replace('formal C11','formal C14').replace('1792921721.8908198','1792952301.9907353')
start=s.index('def run(name,*args):');end=s.index('def formal_health(',start)
s=s[:start]+'''def retry_record(label,ordinal,error):
    rows=STATE.setdefault('observer_retries',[])
    if len(rows)>=64:raise RuntimeError('bounded observer retry journal exceeded')
    rows.append(dict(unix=time.time(),operation=label,attempt=ordinal,error_type=type(error).__name__,error=str(error)[:600]));save()

def run(name,*args,read_only=False):
    return run_json([sys.executable,'-B','-X','utf8',str(B/name),*args],read_only=read_only,
        runner=lambda command:subprocess.run(command,cwd=B,capture_output=True,text=True,timeout=200),
        record=lambda n,e:retry_record(name,n,e))

'''+s[end:]
s=s.replace('q=observe_formal()','q=observe_with_retries(observe_formal,read_only=True,record=lambda n,e:retry_record("formal-resource",n,e))')
s=s.replace("profile,'observe');q=v.get", "profile,'observe',read_only=True);q=v.get")
s=s.replace('row=run(name,profile,observation)','row=run(name,profile,observation,read_only=True)')
s=s.replace("row=run(manager,'observe','--profile',profile)","row=run(manager,'observe','--profile',profile,read_only=True)")
before="            return\n        time.sleep(20)"
after="            STATE.setdefault('ordinary_results',{}).setdefault(profile,{})[str(expected)]=row;save()\n            return\n        time.sleep(20)"
if s.count(before)!=1:raise RuntimeError('exact ordinary proof retention anchor differs')
s=s.replace(before,after)
s=s.replace('acceptance_changed=False,completed_profiles=[])',"acceptance_changed=False,completed_profiles=[],local_pid=os.getpid(),local_namespace='Windows',graph_source_type_modes=['0','1','0'],TopK_mode_constant=1)")
ast.parse(s);name='run_graph_native_model_aba_v375.frozen.py'
with (BASE/name).open('x') as f:f.write(s)
with (BASE/'xolotl-graph-model-controller-v375-prepared.json').open('x') as f:f.write(json.dumps(dict(file=name,bytes=len((BASE/name).read_bytes()),sha256=hashlib.sha256((BASE/name).read_bytes()).hexdigest(),formal_mutations=False,model_gates_weakened=False,readonly_observer_retries=True,mutation_retries=False),indent=2)+'\n')
print(json.dumps(dict(prepared=True,acceptance_changed=False)))
