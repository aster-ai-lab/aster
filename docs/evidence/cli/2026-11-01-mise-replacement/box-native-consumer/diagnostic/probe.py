from pathlib import Path
import hashlib,json,os,subprocess,time,zipfile

root=Path.cwd()
out=root/'.tmp/box-plugin-vm-probe'
sha=lambda data:hashlib.sha256(data).hexdigest()
aix=root/'.tmp/plugins-box-maintained/qualified-final/aster-box-0.1.0-darwin-arm64.aix'
assert sha(aix.read_bytes())=='05e32dd6d1270cb02195a156c138c650406102c98fc5252873d3b0d3a4e761f6'
with zipfile.ZipFile(aix) as package:
    payload=package.read('payload/aster-box-darwin-arm64.tar.gz')
assert sha(payload)=='73e341931c6d8e3171b114a1ccb3694895cb1fb3d949c96bbb3256f510d47ef7'
archive=out/'runtime.tar.gz'
archive.write_bytes(payload)
runtime=out/'runtime'
assert not runtime.exists()
runtime.mkdir()
subprocess.run(['/usr/bin/tar','-xzf',str(archive),'-C',str(runtime)],check=True)
work=out/'work'
work.mkdir()
home=out/'home'
home.mkdir()
os_home=out/'os-home'
os_home.mkdir()
cli=runtime/'aster-box/bin/aster-box'
environment={'PATH':'/usr/bin:/bin:/usr/sbin:/sbin','HOME':str(os_home),'ASTER_HOME':str(home),'RUST_LOG':'warn','LANG':'en_US.UTF-8'}
checks=[]
def run(name,args,timeout):
    started=time.monotonic()
    try:
        p=subprocess.run([str(cli),*args],cwd=work,env=environment,capture_output=True,text=True,encoding='utf-8',timeout=timeout)
        row={'name':name,'args':args,'exitCode':p.returncode,'stdout':p.stdout,'stderr':p.stderr,'timeoutSeconds':timeout,'seconds':time.monotonic()-started}
    except subprocess.TimeoutExpired as error:
        def decode(value):return value.decode('utf-8',errors='replace') if isinstance(value,bytes) else value or ''
        row={'name':name,'args':args,'exitCode':None,'timeout':True,'stdout':decode(error.stdout),'stderr':decode(error.stderr),'timeoutSeconds':timeout,'seconds':time.monotonic()-started}
    checks.append(row)
    (out/'checks.json').write_text(json.dumps({'aixSha256':sha(aix.read_bytes()),'runtimeSha256':sha(payload),'environment':environment,'checks':checks,'packageOwnerIntegrationClaimed':False,'published':False},indent=2)+'\n',encoding='utf-8')
    print(name,row['exitCode'],flush=True)
    return row

version=run('version',['--version'],10)
assert version['exitCode']==0 and version['stdout'].strip()=='aster-box 3.2.7'
image='docker.io/library/alpine:3.22.1'
pull=run('pull',['pull','--platform','linux/arm64',image],120)
if pull['exitCode']!=0:raise SystemExit(1)
inspect=run('image-inspect',['image-inspect',image],10)
assert inspect['exitCode']==0
metadata=json.loads(inspect['stdout'])[0]
digest=metadata['Digest']
assert digest.startswith('sha256:') and len(digest)==71
pinned='docker.io/library/alpine@'+digest
(out/'image.json').write_text(json.dumps({'inputReference':image,'selectedDigest':digest,'pinnedReference':pinned,'inspect':metadata},indent=2)+'\n',encoding='utf-8')
try:
    result=run('guest-command',['run','--rm','--no-stdin','--timeout','30','--network','none','--name','plugin-box-vm-probe',pinned,'--','/bin/sh','-c','printf "aster-box-plugin-vm-ok\\n"; uname -s; uname -m'],120)
finally:
    cleanup=run('cleanup',['rm','--force','plugin-box-vm-probe'],30)
    listing=run('ps',['ps','--all','--format','json'],10)
assert result['exitCode']==0,result
assert 'aster-box-plugin-vm-ok\nLinux\naarch64\n' in result['stdout'],result
assert listing['exitCode']==0
assert json.loads(listing['stdout'])==[]
(out/'accepted.json').write_text(json.dumps({'nativeVmExecutionAccepted':True,'packageOwnerIntegrationClaimed':False,'imageDigest':digest,'aixSha256':sha(aix.read_bytes()),'checks':checks},indent=2)+'\n',encoding='utf-8')
print('Packaged native Box VM execution passed',flush=True)
