"""Resume the exact existing SIMD client after its observer timeout; no replays."""
import hashlib
import json
import time
from xolotl_trial_common import BASE

old_path = BASE / 'xolotl-phase-simd-v140-controller.json'
old = json.loads(old_path.read_text())
basic = json.loads((BASE / 'xolotl-phase-simd-v139-simd-qualification-latest.json').read_text())
native = json.loads((BASE / 'xolotl-native-phase-simd-v138-latest.json').read_text())
if (old['phase'] != 'requires_inspection' or old['current_profile'] != 'simd'
        or [r['profile'] for r in old['profiles']] != ['control_a']
        or basic['exact_client_alive'] or basic['result']['phase'] != 'passed' or len(basic['result']['tests']) != 28
        or native['exact_link_process_alive'] or native['native-link-result.json']['phase'] != 'linked'):
    raise RuntimeError('exact resumed comparison proof differs')
source_path = BASE / 'run_phase_simd_matrix_v140.py'
source = source_path.read_text()
source = source.replace("PATH = BASE / 'xolotl-phase-simd-v140-controller.json'", "PATH = BASE / 'xolotl-phase-simd-v140r1-controller.json'")
start = source.index('STATE = dict(')
end = source.index("with PATH.open('x')", start)
source = source[:start] + '''STATE = json.loads((BASE / 'xolotl-phase-simd-v140-controller.json').read_text())
STATE.update(unix=time.time(), phase='resuming_exact_existing_simd',
             resumed_without_model_restart=True, replayed_model_requests=0,
             prior_observer_error=STATE.pop('error'), prior_finished_unix=STATE.pop('finished_unix'))
''' + source[end:]
source = source.replace('def run(script, *args, timeout=100):', 'def run(script, *args, timeout=200):')
start = source.index('    deadline = time.monotonic() + 3000\n', source.index('\ntry:\n'))
end = source.index("    for profile in ['control_a', 'simd', 'control_b']:", start)
source = source[:start] + source[end:]
source = source.replace("for profile in ['control_a', 'simd', 'control_b']:", "for profile in ['simd', 'control_b']:")
before = "        run('launch_phase_simd_service_v139.frozen.py', profile, timeout=180)"
after = "        if not (BASE / ('xolotl-phase-simd-v139-' + profile + '-intent.json')).exists():\n            run('launch_phase_simd_service_v139.frozen.py', profile, timeout=180)"
if source.count(before) != 1:
    raise RuntimeError('exact fresh-service launch site differs')
source = source.replace(before, after)
before = "        run('manage_phase_simd_basic_v139.frozen.py', profile, 'launch_client')"
after = "        if not (BASE / ('xolotl-phase-simd-v139-' + profile + '-qualification-intent.json')).exists():\n            run('manage_phase_simd_basic_v139.frozen.py', profile, 'launch_client')"
if source.count(before) != 1:
    raise RuntimeError('exact basic launch site differs')
source = source.replace(before, after)
before = "        run('manage_phase_simd_extended_v139.frozen.py', profile, 'launch')"
after = "        if not (BASE / ('xolotl-phase-simd-extended-v139-' + profile + '-intent.json')).exists():\n            run('manage_phase_simd_extended_v139.frozen.py', profile, 'launch')"
if source.count(before) != 1:
    raise RuntimeError('exact extended launch site differs')
source = source.replace(before, after)
compile(source, 'bounded-exact-SIMD-resume-v140r1', 'exec')
new = BASE / 'run_phase_simd_matrix_v140r1.frozen.py'
with new.open('xb') as output:
    output.write(source.encode())
receipt = dict(unix=time.time(), original_controller_sha256=hashlib.sha256(old_path.read_bytes()).hexdigest(),
               original_script_sha256=hashlib.sha256(source_path.read_bytes()).hexdigest(), resumed_script_sha256=hashlib.sha256(new.read_bytes()).hexdigest(),
               original_unified_session=7098, original_observer_terminal_exit=1, exact_existing_simd_basic_passed=True,
               original_model_client_restarted=False, original_basic_replayed=False,
               readonly_observation_subprocess_budget_seconds=200, goal_complete=False)
with (BASE / 'xolotl-phase-simd-resume-v140r1-proof.json').open('x') as output:
    output.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(dict(phase='prepared', resumed_without_model_restart=True, basic_requests_replayed=0)))
