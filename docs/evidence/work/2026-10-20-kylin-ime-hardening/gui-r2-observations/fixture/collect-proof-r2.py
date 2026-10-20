import json,os,subprocess
from pathlib import Path
root=Path('/aster/target/evaluations/kylin-ime-gui');scope=Path((root/'last-run').read_text().strip())
launcher=int((scope/'app-launcher.pid').read_text())
seen=set(); pending=[launcher]; apps=[]
while pending:
 pid=pending.pop()
 if pid in seen: continue
 seen.add(pid)
 try:
  if Path(f'/proc/{pid}/comm').read_text().strip()=='aster-dream': apps.append(pid)
  pending.extend(int(x) for x in Path(f'/proc/{pid}/task/{pid}/children').read_text().split())
 except FileNotFoundError: pass
assert len(apps)==1,apps
pid=apps[0]
keys={b'A3S_STUDIO_INSTANCE',b'XDG_CONFIG_HOME',b'XDG_DATA_HOME',b'XDG_CACHE_HOME',b'XDG_STATE_HOME',b'XDG_RUNTIME_DIR',b'DISPLAY',b'GTK_IM_MODULE',b'GTK_IM_MODULE_FILE',b'FCITX_DBUS_ADDRESS',b'WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS',b'FCITX_ENABLE_SYNC_MODE',b'FCITX_DISABLE_SNOOPER',b'IBUS_DISABLE_SNOOPER',b'LD_PRELOAD',b'A3S_GATEWAY_ORIGIN'}
env={k.decode():v.decode() for item in Path(f'/proc/{pid}/environ').read_bytes().split(b'\0') if b'=' in item for k,v in [item.split(b'=',1)] if k in keys}
proof={'app_pid':pid,'synthetic_session_fixture':True,'expected_commit':(scope/'expected-commit.txt').read_text().strip(),'app_selected_environment':env,'namespaces':{kind:os.readlink(f'/proc/{pid}/ns/{kind}') for kind in ['net','mnt']},'routes':json.loads(subprocess.check_output(['nsenter','--target',str(pid),'--net','ip','-json','route','show'])),'interfaces':json.loads(subprocess.check_output(['nsenter','--target',str(pid),'--net','ip','-json','link','show'])),'profile_mounts':[line for line in Path(f'/proc/{pid}/mountinfo').read_text().splitlines() if '/root/.var/app/dev.a3s.AsterDream' in line],'display_mapping':':150 host Xvfb → :99.0 Flatpak','observation_method':'X11 screenshots; optional separately declared passive DOM metadata observer; no DOM edits or event dispatch'}
(scope/'startup-proof.json').write_text(json.dumps(proof,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'app_pid':pid,'routes':proof['routes'],'interface_names':[item['ifname'] for item in proof['interfaces']],'sync_mode':env.get('FCITX_ENABLE_SYNC_MODE')}))
