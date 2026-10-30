"""Bind the finite read-only readiness observer to the exact new formal C11."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE
text=(BASE/'launch_xolotl_readiness_watch_v99.frozen.py').read_text()
text=text.replace('xolotl-readiness-watch-v99','xolotl-readiness-watch-v159').replace('/readiness-watch-v99','/readiness-watch-v159')
text=text.replace('xolotl-stable-service-v97-intent.json','xolotl-stable-service-v157-intent.json').replace('xolotl-stable-recovery-v97-promotion.json','xolotl-stable-recovery-v157-promotion.json')
ast.parse(text)
path=BASE/'launch_xolotl_readiness_watch_v159.frozen.py'
with path.open('x') as output:output.write(text)
print(json.dumps(dict(phase='prepared',read_only=True,worker_changed=False,operator_sha256=hashlib.sha256(path.read_bytes()).hexdigest())))
