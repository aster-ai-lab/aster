import json,os,pathlib,subprocess,time
root=pathlib.Path(__file__).resolve().parents[4]
out=pathlib.Path(__file__).resolve().parent
commands=json.loads((out/'validation-commands.json').read_text())
environment=os.environ.copy()
environment['PATH']=str(pathlib.Path.home()/'.cargo/bin')+':'+environment.get('PATH','')
environment.update(CARGO_TARGET_DIR='/code/aster/target',CARGO_BUILD_JOBS='2',CARGO_NET_OFFLINE='false',CARGO_TERM_COLOR='never',PYTHONDONTWRITEBYTECODE='1')
results=json.loads((out/'validation-results.json').read_text()) if (out/'validation-results.json').exists() else []
for name,command in commands:
 started=time.time()
 print('START',name,flush=True)
 with (out/(name+'.log')).open('w') as log:
  log.write('cwd='+str(root)+'\ncommand='+repr(command)+'\n');log.flush()
  process=subprocess.run(command,cwd=root,env=environment,stdout=log,stderr=subprocess.STDOUT)
 result={'name':name,'command':command,'exit_code':process.returncode,'wall_seconds':time.time()-started}
 results.append(result)
 (out/'validation-results.json').write_text(json.dumps(results,indent=2)+'\n')
 print('DONE',name,process.returncode,flush=True)
 if process.returncode!=0:raise SystemExit(process.returncode)
print('ALL_VALIDATIONS_PASSED',flush=True)
