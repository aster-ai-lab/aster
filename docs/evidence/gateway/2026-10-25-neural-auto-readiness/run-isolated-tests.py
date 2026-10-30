import json,subprocess,pathlib,os,time
out=pathlib.Path(__file__).parent
cfg=json.loads((out/'isolated-test-config.json').read_text())
env=os.environ.copy();env['ORT_DYLIB_PATH']='/opt/a3s/lib/libonnxruntime.so'
results=[]
for name,args in cfg['commands']:
 started=time.monotonic()
 with (out/(name+'.log')).open('w') as log:
  result=subprocess.run([cfg['binary']]+args,cwd=cfg['root'],env=env,stdout=log,stderr=subprocess.STDOUT,timeout=300)
 results.append({'name':name,'arguments':args,'exit_code':result.returncode,'seconds':time.monotonic()-started})
 (out/'isolated-test-results.json').write_text(json.dumps(results,indent=2)+'\n')
