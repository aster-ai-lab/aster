import json,subprocess,pathlib,os
out=pathlib.Path(__file__).parent
cfg=json.loads((out/'annotation-test-config.json').read_text())
env=os.environ.copy();env['ORT_DYLIB_PATH']='/opt/a3s/lib/libonnxruntime.so';env['CARGO_TARGET_DIR']='/code/aster/target';env['CARGO_BUILD_JOBS']='2';env['CARGO_NET_OFFLINE']='true'
results=[]
for name,pattern in [('annotation-requests','inference::openai_request'),('annotation-e2e','neural_routing_tests')]:
 with (out/(name+'.log')).open('w') as log:r=subprocess.run(cfg['command']+[pattern,'--','--test-threads=1'],cwd=cfg['root'],env=env,stdout=log,stderr=subprocess.STDOUT)
 results.append({'name':name,'filter':pattern,'exit_code':r.returncode})
 (out/'annotation-test-results.json').write_text(json.dumps(results,indent=2)+'\n')
 if r.returncode:raise SystemExit(r.returncode)
