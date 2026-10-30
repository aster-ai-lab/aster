"""Keep earlier frozen scripts, use the exact resumed matrix proof and bounded file transport."""
import hashlib
import json
from xolotl_trial_common import BASE

output=[]
for parent,name,label,before,after in [
    ('launch_native_profile_v117.frozen.py','launch_native_profile_v117r1.frozen.py','native-profile-v117r1',
     "record['process']=owned(code)","record['process']=sealed_owned(code,'native-profile-v117r1')"),
    ('launch_code_service_v118.frozen.py','launch_code_service_v118r1.frozen.py','code-service-v118r1',
     "record.update(process=owned(code),phase='launched')","record.update(process=sealed_owned(code,'code-service-v118r1'),phase='launched')")]:
    source=(BASE/parent).read_text()
    if source.count('xolotl-pool-seed-v112r1-controller.json')!=1 or source.count(before)!=1:
        raise RuntimeError('pinned pending launch boundary differs')
    source=source.replace('xolotl-pool-seed-v112r1-controller.json','xolotl-pool-seed-v112r2-controller.json')
    source=source.replace(before,after)
    if source.count('from xolotl_trial_common import ')!=1:raise RuntimeError('pinned transport import boundary differs')
    index=source.index('\n',source.index('from xolotl_trial_common import '))
    source=source[:index+1]+'from xolotl_sealed_bootstrap_v120 import sealed_owned\n'+source[index+1:]
    compile(source,name,'exec')
    with (BASE/name).open('x',encoding='utf-8',newline='\n') as handle:handle.write(source)
    output.append(dict(file=name,sha256=hashlib.sha256((BASE/name).read_bytes()).hexdigest(),
        parent=parent,parent_sha256=hashlib.sha256((BASE/parent).read_bytes()).hexdigest(),same_native_bootstrap_inputs=True))
with (BASE/'xolotl-file-launches-v120-prepared.json').open('x') as handle:
    handle.write(json.dumps(dict(files=output,gpu_launched=False,service_changed=False),indent=2)+'\n')
print(json.dumps(dict(prepared_files=len(output),gpu_launched=False)))
