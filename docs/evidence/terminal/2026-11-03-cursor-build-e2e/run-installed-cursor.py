from pathlib import Path
import hashlib,json,os,subprocess,tempfile,uuid
root=Path.cwd(); pair=root/'target/cursor-build-e2e-20261103/installed'; evidence=root/'docs/evidence/terminal/2026-11-03-cursor-build-e2e'
prompt='Use read to read input.txt, write its exact bytes to output.txt with write, read output.txt, then use bash once to run `test -f output.txt && cmp input.txt output.txt`. After the command succeeds, reply with the single line from input.txt followed by a space and verified. Do not delegate or run other commands.'
results=[]
for model in ['auto','composer-2.5']:
    with tempfile.TemporaryDirectory(prefix='aster-cursor-delivery-') as temporary:
        tmp=Path(temporary); workspace=tmp/'workspace 空格'; workspace.mkdir(); home=tmp/'aster-home'; home.mkdir()
        data=f'cursor-delivery-{uuid.uuid4()}\n'.encode(); (workspace/'input.txt').write_bytes(data)
        config=tmp/'config.acl'; config.write_text(f'import_accounts = ["cursor"]\ndefault_model = "cursor/{model}"\n')
        env={k:os.environ[k] for k in ['HOME','PATH','TMPDIR','LANG','LC_ALL'] if k in os.environ}
        env.update(ASTER_HOME=str(home),GROK_HOME=str(home),ASTER_CONFIG=str(config),ASTER_CONFIG_FILE=str(config),ASTER_DISABLE_GROK_ACCOUNT='1',ASTER_ACP_AGENT_BIN=str(pair/'aster-acp'),TERM='xterm-256color')
        command=[str(pair/'aster'),'--single',prompt,'--no-plan','--no-auto-update','--permission-mode','auto','--max-turns','8']
        log=evidence/f'cursor-cli-{model}.log'
        try:
            with log.open('w') as output:
                process=subprocess.run(command,cwd=workspace,env=env,stdout=output,stderr=subprocess.STDOUT,timeout=240)
            text=log.read_text(); actual=(workspace/'output.txt').read_bytes() if (workspace/'output.txt').is_file() else None
            passed=process.returncode==0 and actual==data and (workspace/'input.txt').read_bytes()==data and data.decode().strip()+' verified' in text and 'Turn failed' not in text
            result={'model':'cursor/'+model,'passed':passed,'exit_code':process.returncode,'expected_sha256':hashlib.sha256(data).hexdigest(),'actual_sha256':hashlib.sha256(actual).hexdigest() if actual is not None else None,'log':log.name}
        except subprocess.TimeoutExpired:
            result={'model':'cursor/'+model,'passed':False,'error':'240-second task timeout','log':log.name}
        results.append(result); print(json.dumps(result),flush=True)
        (evidence/'cursor-cli-results.json').write_text(json.dumps({'prompt':prompt,'timeout_seconds':240,'cases':results},indent=2)+'\n')
raise SystemExit(0 if all(case['passed'] for case in results) else 1)
