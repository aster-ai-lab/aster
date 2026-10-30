"""Freeze an off/on/off-repeat single-binary comparison without changing any output gate."""
import ast
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent


def one(s, old, new):
    if s.count(old) != 1:
        raise RuntimeError('exact frozen source anchor differs: ' + old[:120])
    return s.replace(old, new)


def common(s):
    for old, new in [
        ("['baseline','draft_off','draft_off_ref','draft_on_serial']", "['off','on','off_repeat']"),
        ('xolotl-isolation-service-v89r1-', 'xolotl-admission-service-v101-'),
        ('serve-isolation-v89r1-', 'serve-admission-v101-'),
        ('xolotl-isolation-client-v90r1-', 'xolotl-admission-client-v102-'),
        ('qualification-isolation-v90', 'qualification-admission-v102'),
        ('launch_xolotl_isolation_v89r1.frozen.py', 'launch_admission_seed_service_v101.frozen.py'),
        ('manage_xolotl_isolation_v91r1.frozen.py', 'manage_admission_seed_matrix_v102.frozen.py'),
        ('xolotl-isolation-v92r1-controller.json', 'xolotl-admission-seed-v103-controller.json'),
    ]:
        s = s.replace(old, new)
    return s


def freeze(name, s):
    ast.parse(s)
    with (B / name).open('xb') as f:
        f.write(s.encode())
    return dict(file=name, bytes=len(s.encode()), sha256=hashlib.sha256(s.encode()).hexdigest())


proof = json.loads((B / 'xolotl-admission-seed-native-v98-latest.json').read_text())
if proof['result']['phase'] != 'linked' or proof['exact_link_process_alive']:
    raise RuntimeError('exact controlled native link must finish')
binary_sha = proof['result']['binary']['sha256']
s = common((B / 'launch_xolotl_isolation_v89r1.frozen.py').read_text())
s = s.replace('xolotl-draft-cumulative-v49-native-link-intent.json', 'xolotl-admission-seed-native-v98-intent.json')
s = s.replace('3a66564e-0a12-4c17-9b41-741fd78e0017', '7455d989-8255-4f7c-ae8f-3cc0aa5928e4')
s = s.replace('xolotl-stable-service-v58-intent.json', 'xolotl-stable-service-v97-intent.json')
s = s.replace('34f3c963304445e50c4ae398194f4b4745a0b1868a6b4a1d6668068aebcf96e6', binary_sha)
s = s.replace('exact independent C9 allocation', 'exact independent C10 allocation')
s = s.replace('Exact native34f/API f062/S512, configured1M; new GPU hardware passed, diagnostic model gates pending',
              'Controlled admission-seed c96/API f062/S512/GPFS, configured1M; flag off/on/off repeat, no adoption')
begin = s.index("env=dict(plan['engine_env'],STRATA_ROWS_REF=")
end = s.index("plan['engine_env']=env", begin)
s = s[:begin] + "env=dict(plan['engine_env'],STRATA_ROWS_REF='0',STRATA_BATCH_SERIAL='0',STRATA_ENGRAM_CACHE_ROWS='2000000',ASTER_XQ_ADMIT_DRAFT_SEED='1' if profile=='on' else '0')\n" + s[end:]
s = s.replace('copy_may_overlap_diagnostics=True', 'copy_may_overlap_diagnostics=False')
s = one(s, "slots_serial=env['STRATA_BATCH_SERIAL']", "slots_serial=env['STRATA_BATCH_SERIAL'],admission_seed=env['ASTER_XQ_ADMIT_DRAFT_SEED']")
receipts = [freeze('launch_admission_seed_service_v101.frozen.py', s)]

s = common((B / 'manage_xolotl_isolation_v91r1.frozen.py').read_text())
receipts.append(freeze('manage_admission_seed_matrix_v102.frozen.py', s))

s = common((B / 'run_xolotl_isolation_v92r1.frozen.py').read_text())
old = """        if profile=='baseline':
            old=json.loads(path.read_text())
            if old['phase']!='launched' or not old.get('process'):raise RuntimeError('exact existing baseline launch handle required')
        else:run('launch_admission_seed_service_v101.frozen.py',profile)
"""
s = one(s, old, "        run('launch_admission_seed_service_v101.frozen.py',profile)\n")
s = s.replace('Finite four-profile diagnostic', 'Finite off/on/off-repeat admission-seed diagnostic')
receipts.append(freeze('run_admission_seed_matrix_v103.frozen.py', s))

with (B / 'xolotl-admission-seed-v101-preparation.json').open('x') as f:
    f.write(json.dumps(dict(binary_sha256=binary_sha, profiles=['off','on','off_repeat'],
        unchanged_request_worker='xolotl_isolation_worker_v90.frozen.py',
        qualified_output_gate_unchanged=True, files=receipts), indent=2) + '\n')
print(json.dumps(dict(phase='prepared', profiles=3, requests_per_profile=10, binary_sha256=binary_sha)))
