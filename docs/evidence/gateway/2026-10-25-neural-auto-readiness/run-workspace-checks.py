import json,os,pathlib,subprocess,time
out=pathlib.Path(__file__).parent
cfg=json.loads((out/'workspace-check-config.json').read_text())
results=[]
for name,command in cfg['commands']:
 started=time.monotonic()
 with (out/(name+'.log')).open('w') as log:
  r=subprocess.run(command,cwd=cfg['root'],stdout=log,stderr=subprocess.STDOUT,env=os.environ.copy())
 results.append({'name':name,'command':command,'exit_code':r.returncode,'seconds':time.monotonic()-started})
 (out/'workspace-check-results.json').write_text(json.dumps(results,indent=2)+'\n')
