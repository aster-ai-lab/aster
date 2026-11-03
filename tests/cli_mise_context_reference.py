"""Observe native context requests with the frozen mise binary and owned files."""
import argparse
import hashlib,json,os,platform,signal,subprocess,tempfile
from pathlib import Path

root=Path(__file__).resolve().parents[1]
contracts=root/'specs/031-cli-mise-replacement/contracts'
reference=json.loads((contracts/'mise-cli-reference.json').read_text())
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--mise',type=Path,required=True)
parser.add_argument('--output',type=Path,default=contracts/'mise-context-reference.json')
options=parser.parse_args()
binary=options.mise.resolve()
with binary.open('rb') as source:
    assert hashlib.file_digest(source,'sha256').hexdigest()==reference['binary_sha256']
assert os.name=='posix'
cases=[
    ('absolute-path','argument','absolute',False),
    ('relative-argument','argument','relative',False),
    ('ancestor-config','project','relative',False),
    ('home-path','argument','home',False),
    ('system-with-managed-shim','argument','system',True),
    ('missing-path','argument','missing',False),
    ('global-relative-path','global','relative',False),
]
results=[]
with tempfile.TemporaryDirectory(prefix='aster-mise-context-reference-') as temporary:
    base=Path(temporary)
    for number,(name,source,kind,poison_shim) in enumerate(cases):
        case=base/str(number)
        for directory in ['home','config','data/shims','cache','system','project/nested','system-bin']:
            (case/directory).mkdir(parents=True)
        cwd=case/'project/nested'
        external=(case/'home/SDK node') if kind=='home' or source=='global' else ((case/'project/external node') if source=='project' else cwd/'external node')
        (external/'bin').mkdir(parents=True)
        path_node=external/'bin/node'
        path_node.write_text('#!/bin/sh\nprintf \'%s\\n\' \'external-node\'\n')
        path_node.chmod(0o755)
        for destination,word in [(case/'system-bin/node','system-node'),(case/'data/shims/node','managed-shim')]:
            destination.write_text(f"#!/bin/sh\nprintf '%s\\n' '{word}'\n")
            destination.chmod(0o755)
        request={'absolute':'path:'+str(external),'relative':'path:./'+external.name,'home':'path:~/SDK node','system':'system','missing':'path:./missing node'}[kind]
        environment={'PATH':os.pathsep.join(([str(case/'data/shims')] if poison_shim else [])+[str(case/'system-bin'),'/usr/bin','/bin']), 'HOME':str(case/'home'),'MISE_CONFIG_DIR':str(case/'config'),'MISE_DATA_DIR':str(case/'data'),'MISE_CACHE_DIR':str(case/'cache'),'MISE_SYSTEM_CONFIG_DIR':str(case/'system'),'MISE_GLOBAL_CONFIG_FILE':str(case/'config/global.toml'),'MISE_GLOBAL_CONFIG_ROOT':str(case/'home'),'MISE_PARANOID':'1','MISE_YES':'1','NO_COLOR':'1'}
        def run(args):
            child=subprocess.Popen([str(binary),*args],cwd=cwd,env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8',start_new_session=True)
            try:
                stdout,stderr=child.communicate(timeout=25)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid,signal.SIGKILL)
                stdout,stderr=child.communicate()
                raise RuntimeError(f'reference timeout for {name}: {stderr}')
            return {'exit':child.returncode,'stdout':stdout,'stderr':stderr}
        config=None
        if source=='project':
            config=case/'project/.mise/config.toml'
        elif source=='global':
            config=case/'config/global.toml'
        if config:
            config.parent.mkdir(parents=True,exist_ok=True)
            config.write_text('[tools]\nnode = '+json.dumps(request)+'\n',encoding='utf-8')
            trusted=run(['trust',str(config)])
            assert trusted['exit']==0,trusted
        args=['exec']+(['node@'+request] if source=='argument' else [])+['--','node']
        result=run(args)
        results.append({'name':name,'source':source,'kind':kind,'command':['mise',*args],'request':request,'managed_shim_in_path':poison_shim,**result})
        print(json.dumps({'case':name,**result}),flush=True)
artifact={'tag':reference['tag'],'release_commit':reference['release_commit'],'binary_sha256':reference['binary_sha256'],'host':{'os':platform.system(),'arch':platform.machine()},'timeout_seconds':25,'paranoid':True,'cases':results}
path=options.output
path.parent.mkdir(parents=True,exist_ok=True)
path.write_text(json.dumps(artifact,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
assert json.loads(path.read_text())==artifact
