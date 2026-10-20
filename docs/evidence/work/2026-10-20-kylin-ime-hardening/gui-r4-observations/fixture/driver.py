import json, os, subprocess, sys, time
from pathlib import Path
root=Path('/aster/target/evaluations/kylin-ime-gui')
scope=Path((root/'last-run').read_text().strip())
os.environ.update(json.loads((scope/'environment.json').read_text()))
os.environ.pop('LD_LIBRARY_PATH',None)
args=sys.argv[1:]
if args[0]=='observe':
 subprocess.run(['timeout','20','python3',str(root/'observe.py'),str(scope/args[1])],check=True)
else:
 if args[0]=='focus':
  windows=subprocess.check_output(['xdotool','search','--onlyvisible','--name','^AsterDream$'],text=True).split()
  assert len(windows)==1, windows
  cmd=['xdotool','windowfocus','--sync',windows[0]]
 elif args[0]=='click': cmd=['xdotool','mousemove',args[1],args[2],'click','1']
 elif args[0]=='keys': cmd=['xdotool','key','--clearmodifiers','--delay','120',*args[1:]]
 elif args[0]=='hold-return':
  down=subprocess.run(['xdotool','keydown','Return'],capture_output=True,text=True,timeout=5)
  down.check_returncode()
  started=time.monotonic()
  time.sleep(1.0)
  cmd=['xdotool','keyup','Return']
  with (scope/'actions.jsonl').open('a') as out: out.write(json.dumps({'action':args,'keydown_returncode':down.returncode,'held_ms':round((time.monotonic()-started)*1000),'hold_ms_requested':1000})+'\n')
 elif args[0]=='quick-confirm-send':
  cmds=[['xdotool','keydown','Return'],['xdotool','keyup','Return'],['xdotool','keydown','Return'],['xdotool','keyup','Return']]
  times=[]
  for index, command in enumerate(cmds):
   times.append(time.monotonic_ns())
   result=subprocess.run(command,capture_output=True,text=True,timeout=5)
   result.check_returncode()
   if index in [0,2]: time.sleep(0.04)
   if index==1: time.sleep(0.3)
  record={'action':args,'commands':cmds,'monotonic_ns_before_command':times,'press_gap_ms':(times[2]-times[0])/1000000,'key_hold_requested_ms':40,'release_gap_requested_ms':300,'returncode':0}
  with (scope/'actions.jsonl').open('a') as out: out.write(json.dumps(record)+'\n')
  print(json.dumps(record))
  raise SystemExit(0)
 elif args[0]=='im-state': cmd=['fcitx-remote']
 elif args[0]=='im-open': cmd=['fcitx-remote','-o']
 elif args[0]=='im-close': cmd=['fcitx-remote','-c']
 elif args[0]=='accessibility': cmd=['gdbus','call','--session','--dest','org.a11y.Bus','--object-path','/org/a11y/bus','--method','org.freedesktop.DBus.Properties.Set','org.a11y.Status','IsEnabled','<true>']
 else: raise SystemExit('unknown fixture action')
 result=subprocess.run(cmd,capture_output=True,text=True,timeout=20)
 record={'action':args,'command':cmd,'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
 with (scope/'actions.jsonl').open('a') as out: out.write(json.dumps(record,ensure_ascii=False)+'\n')
 print(json.dumps(record,ensure_ascii=False))
 result.check_returncode()
