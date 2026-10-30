"""Use identical numeric aggregation for the separately proven native --batch2 trace."""
import hashlib
import json
from xolotl_trial_common import BASE

source=(BASE/'xolotl_profile_statistics_worker_v123.frozen.py').read_text()
before="        prefill_chunk_tokens=q['plan']['prefill_chunk_tokens'],gpu_profiled_requests=1,"
after="        prefill_chunk_tokens=q['plan']['prefill_chunk_tokens'],parallel_slots=q['plan']['parallel_slots'],native_info=q['native_info'],gpu_profiled_requests=1,"
if source.count(before)!=1:raise RuntimeError('pinned numeric profile metadata anchor differs')
source=source.replace(before,after)
compile(source,'xolotl_profile_statistics_worker_v126.frozen.py','exec')
with (BASE/'xolotl_profile_statistics_worker_v126.frozen.py').open('x',encoding='utf-8',newline='\n') as output:output.write(source)
manager=(BASE/'manage_profile_statistics_v123.frozen.py').read_text()
manager=manager.replace('xolotl-native-profile-v117-intent.json','xolotl-native-profile-v125-intent.json')
manager=manager.replace('xolotl-native-statistics-v123-','xolotl-native-statistics-v126-')
manager=manager.replace('xolotl_profile_statistics_worker_v123.frozen.py','xolotl_profile_statistics_worker_v126.frozen.py')
compile(manager,'manage_profile_statistics_v126.frozen.py','exec')
with (BASE/'manage_profile_statistics_v126.frozen.py').open('x',encoding='utf-8',newline='\n') as output:output.write(manager)
print(json.dumps(dict(prepared=True,gpu_launched=False,statistics_launched=False)))
