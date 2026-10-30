"""Freeze a fresh read-only formal check without the older checkpoint mutation."""
import ast
from xolotl_trial_common import BASE
source = (BASE/'observe_formal_and_checkpoint_v113.py').read_text().split("checkpoint=BASE/'xolotl-serving-and-optimization-next-iteration.json'",1)[0]
source = source.replace('xolotl-formal-health-v113.json', 'xolotl-formal-health-v152.json')
source += "print(json.dumps(dict(formal_phase=resource['phase'],health=health,readiness_observer_alive=q['exact_observer_alive'],inference_sent=False,service_restarted=False)))\n"
ast.parse(source)
with (BASE/'observe_formal_readonly_v152.py').open('x') as output:
    output.write(source)
print('Prepared immutable read-only formal check; no GPU/model mutation.')
