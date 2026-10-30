"""Bind the unchanged finite read-only observer to promoted C13."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE
raw=(BASE/'launch_xolotl_readiness_watch_v159.frozen.py').read_text()
raw=raw.replace('xolotl-readiness-watch-v159','xolotl-readiness-watch-v220').replace('/readiness-watch-v159','/readiness-watch-v220').replace('xolotl-stable-service-v157-intent.json','xolotl-stable-service-v217-intent.json').replace('xolotl-stable-recovery-v157-promotion.json','xolotl-stable-recovery-v217-promotion.json')
ast.parse(raw);path=BASE/'launch_xolotl_readiness_watch_v220.frozen.py'
with path.open('x') as out:out.write(raw)
print(json.dumps(dict(prepared=True,worker_changed=False,observation_only=True,operator_sha256=hashlib.sha256(path.read_bytes()).hexdigest())))
