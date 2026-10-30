"""Observe the exact jump-host capacity client, never infer terminal from timeout."""
import argparse
import json
from xolotl_trial_common import BASE, jump
parser=argparse.ArgumentParser()
parser.add_argument('kind',choices=['remainder','fullwindow'])
kind=parser.parse_args().kind
c=json.loads((BASE/('xolotl-resident-v38-'+kind+'-intent.json')).read_text())
code='''import json,time
from pathlib import Path
c=@C@;root=Path(c['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact V38 client root differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('capacity client receipt bound exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(c['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==c['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@C@',repr(c))
v=jump(code)
(BASE/('xolotl-resident-v38-'+kind+'-latest.json')).write_text(json.dumps(v,indent=2)+'\n')
print(json.dumps(v,indent=2))
