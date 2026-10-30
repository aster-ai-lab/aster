"""Read-only final receipts. Call with authenticated in-memory API helpers."""
import hashlib
import json
import pathlib
import subprocess
import time
import urllib.parse
import urllib.request


def collect_final_state(evidence, admin_api, node_get, bare_key):
    root=pathlib.Path(evidence)
    def load(name):return json.loads((root/name).read_text())
    def save(name,value):(root/name).write_text(json.dumps(value,indent=2))
    def get(url,key=None):
        request=urllib.request.Request(url,headers={'Authorization':'Bearer '+key} if key else {})
        with urllib.request.urlopen(request,timeout=10) as response:
            body=response.read(8*1024*1024+1)
        assert len(body)<=8*1024*1024
        return json.loads(body)
    def sha(path):
        with pathlib.Path(path).open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()
    def props(unit):return dict(line.split('=',1) for line in subprocess.check_output(['systemctl','show',unit,'-p','MainPID','-p','ActiveState'],text=True).splitlines())
    cold=load('native-one-million-optimized/result.json');assert cold['success'] and cold['full_one_million_completed']
    identity=load('native-one-million-optimized/identity-after.json')
    binding=load('candidate-indexer-artifacts.json')
    deployment=load('dspark-coldA-chunk256-indexer64m-deployment.json')
    power=props('a3s-power-deepseek-gpu6.service')
    pid=int(power['MainPID']);engine=identity['engine_pid']
    argv=pathlib.Path(f'/proc/{engine}/cmdline').read_bytes().decode().rstrip('\0').split('\0')
    raw_env=pathlib.Path(f'/proc/{engine}/environ').read_bytes().split(bytes([0]))
    all_env=dict(raw.decode().split('=',1) for raw in raw_env if b'=' in raw)
    expected_env=identity['engine_env']
    keys=['STRATA_SERVE_DSPARK','STRATA_DSPARK_DEPTH','STRATA_DSPARK_CONF','STRATA_SERVE_DSPARK_EVICT','STRATA_INDEXER_TILE']
    current_env={key:all_env[key] for key in keys if key in all_env}
    registry=get('http://127.0.0.1:18206/v1/models',bare_key)['data']
    model=[m for m in registry if m['id']=='deepseek-v41-flash-1017-xq-1m'];assert len(model)==1
    model={k:model[0][k] for k in ['id','object','created','owned_by','context_length','format','size_bytes','sha256']}
    original=load('baseline.json')
    other={unit:props(unit) for unit in deployment['preserved_services_after']}
    placements=[row.split(',')[0].strip() for row in subprocess.check_output(['nvidia-smi','--query-compute-apps=gpu_uuid,pid','--format=csv,noheader'],text=True).splitlines() if row.split(',')[1].strip()==str(engine)]
    selected={'at_epoch':time.time(),'power_pid':pid,'engine_pid':engine,'power_sha256':sha(f'/proc/{pid}/exe'),'engine_sha256':sha(f'/proc/{engine}/exe'),
              'acl_sha256':subprocess.check_output(['sudo','-n','sha256sum','/etc/a3s/power/deepseek-gpu6.acl'],text=True).split()[0],
              'engine_argv':argv,'engine_env':current_env,'same_v3_weights':model['sha256']=='73df1934302f867601dd27f7fca59109d9b11674f50cc60526ec2a1cd4f3d389' and model['size_bytes']==162954903552,
              'other_service_pids_unchanged':other==original['other_services']}
    selected['checks']={'same_power':pid==identity['power_pid'] and selected['power_sha256']==binding['power_sha256'],
                        'same_engine':selected['engine_sha256']==binding['engine_sha256'] and argv==identity['engine_argv'],
                        'same_tuning':current_env==expected_env,'same_acl':selected['acl_sha256']==deployment['new_acl_sha256'],
                        'same_v3_weights':selected['same_v3_weights'],'context_1m':model['context_length']==1048576,
                        'gpu6_only':placements==['GPU-ab93a347-c678-1d71-0e85-0f520463debe'],
                        'other_services_preserved':selected['other_service_pids_unchanged'],
                        'private_content_logs_disabled':all_env.get('STRATA_TEXT_LOG')=='0' and 'STRATA_REQUEST_LINES' not in all_env}
    selected['all_passed']=all(selected['checks'].values());assert selected['all_passed']
    save('final-selected-profile.json',selected);save('final-power-model-registry.json',model)
    journal=json.loads(pathlib.Path('/var/lib/a3s/web/gateway/main-journal.json').read_text())['snapshot']
    snapshot={key:journal[key] for key in ['gateway_id','revision','snapshot_digest']}
    code,node=node_get('/snapshots/status?'+urllib.parse.urlencode(snapshot))
    assert code==200 and node['ready'] and node['state']=='applied'
    save('final-gateway-managed-snapshot.json',{'snapshot':snapshot,'ready':node['ready'],'state':node['state']})
    h=get('http://127.0.0.1:18206/health');n=get('http://127.0.0.1:18106/health');s=get('http://127.0.0.1:18106/status')
    speech={}
    for port in [18300,18301]:
        with urllib.request.urlopen(f'http://127.0.0.1:{port}/health',timeout=5) as response:speech[str(port)]=response.status
    checks={'power_loaded':h['status']=='ok' and h['loaded_models']==1,'admission_idle':h['worker']['admission']['active']==0 and h['worker']['admission']['waiting']==0,
            'native_loaded_1m':n['loaded'] is True and n['max_context']==1048576,'native_idle':s.get('busy') is False and not s.get('queued',0),
            'gateway_ready':node['ready'],'speech_healthy':all(status==200 for status in speech.values()),**selected['checks']}
    health={'at_epoch':time.time(),'checks':checks,'all_passed':all(checks.values()),'power':{'status':h['status'],'loaded_models':h['loaded_models'],'admission':h['worker']['admission']},
            'native':{key:n[key] for key in ['status','loaded','model','max_context']},'snapshot':snapshot,'speech_health':speech,'other_services':other}
    assert health['all_passed'];save('final-runtime-health.json',health)
    mc,me=admin_api('/model-service/models',timeout=10);sc,se=admin_api('/model-service/suppliers',timeout=10)
    assert mc==sc==200
    models=[m for m in me['data'] if m['id']=='deepseek-v4.1-flash'];suppliers=[s for s in se['data'] if s['id']=='c3b37832-2dd9-4931-93dc-6f4626ee0baf']
    assert len(models)==len(suppliers)==1 and suppliers[0]['baseUrl']=='http://127.0.0.1:18206/v1'
    official={'at_epoch':time.time(),'model':{key:models[0].get(key) for key in ['id','name','state','contextWindow','outputLimit','capabilities','unavailableReason','routes']},
              'supplier':{key:suppliers[0].get(key) for key in ['id','name','baseUrl','enabled','hasKey','kind','protocol','routable','models']}}
    assert official['model']['state']=='published' and official['model']['contextWindow']==1048576 and official['supplier']['enabled'] and official['supplier']['routable']
    save('official-model-and-supplier-state-final.json',official)
    return {'selected_profile_passed':True,'runtime_health_passed':True,'official_model_published':True,'gateway_revision':snapshot['revision']}
