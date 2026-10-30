"""Finite same-GPU GPFS/local/GPFS comparison. Never resubmit an existing handle."""
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE

PATH = BASE / 'xolotl-engram-comparison-v71-controller.json'
STATE = dict(schema='aster-xolotl-engram-comparison-v71', unix=time.time(), phase='starting',
             profiles=[], formal_service_unchanged=True, actual1m_input_tested=False,
             scope='Same GPU355/native34f/API f062/S512; only Engram directory changes; fresh processes; source OS cache not evicted')
with PATH.open('x') as output:
    output.write(json.dumps(STATE, indent=2)+'\n')


def save():
    STATE['observed_unix'] = time.time()
    temporary = PATH.with_suffix('.tmp')
    temporary.write_text(json.dumps(STATE, indent=2)+'\n')
    temporary.replace(PATH)


def run(name, *arguments):
    command = [sys.executable, '-X', 'utf8', str(BASE / name), *arguments]
    completed = subprocess.run(command, cwd=BASE, capture_output=True, text=True, timeout=100)
    if len(completed.stdout)+len(completed.stderr)>2*1024**2:
        raise RuntimeError('bounded operator output exceeded')
    if completed.returncode:
        raise RuntimeError(name+' '+str(arguments)+' exit'+str(completed.returncode)+': '+completed.stderr[-3500:])
    return completed.stdout


def wait_service(profile, target, seconds):
    deadline = time.monotonic()+seconds
    while time.monotonic()<deadline:
        run('manage_xolotl_engram_disk_v68.frozen.py', profile, 'observe')
        value = json.loads((BASE / ('xolotl-engram-disk-v68-'+profile+'-latest.json')).read_text())
        state = value.get('service-longctx-v1-result.json', {})
        phase = state.get('phase')
        STATE['service_phase'] = phase
        save()
        if target=='serving' and phase=='serving' and value['exact_supervisor_alive']:
            return state
        if target=='stopped' and phase=='stopped' and state.get('api_reaped') and not value['exact_supervisor_alive']:
            return state
        if phase=='failed' or target=='serving' and not value['exact_supervisor_alive']:
            raise RuntimeError('exact model service failed/disappeared; inspect same handle')
        time.sleep(15)
    raise RuntimeError('service observation deadline reached; inspect same handle without resubmission')


def wait_client(profile, version, seconds):
    deadline = time.monotonic()+seconds
    script = 'manage_xolotl_engram_'+('natural_v69' if version==69 else 'extended_v70')+'.frozen.py'
    path = BASE / ('xolotl-engram-'+('natural-v69' if version==69 else 'extended-v70')+'-'+profile+'-latest.json')
    while time.monotonic()<deadline:
        run(script, profile, 'observe')
        value = json.loads(path.read_text())
        result = value.get('result', {})
        STATE['client_phase'] = result.get('phase')
        STATE['client_tests'] = len(result.get('tests', []))
        save()
        if not value['exact_client_alive']:
            if result.get('phase')!='passed':
                raise RuntimeError('exact comparison client stopped without passing: '+str(result.get('error')))
            return result
        if result.get('phase')=='failed':
            raise RuntimeError('exact comparison failed; wait for same handle before any drain: '+str(result.get('error')))
        time.sleep(20)
    raise RuntimeError('client observation deadline reached; inspect same handle without resubmission')


try:
    for profile in ['gpfs_a', 'local', 'gpfs_b']:
        STATE.update(phase='launching_service', profile=profile)
        save()
        run('launch_xolotl_engram_disk_v68.frozen.py', profile)
        wait_service(profile, 'serving', 300)
        STATE['phase'] = 'natural_requests'
        save()
        run('manage_xolotl_engram_natural_v69.frozen.py', profile, 'launch')
        natural = wait_client(profile, 69, 620)
        STATE['phase'] = 'native_tool_long_output'
        save()
        run('manage_xolotl_engram_extended_v70.frozen.py', profile, 'launch')
        extended = wait_client(profile, 70, 620)
        STATE['profiles'].append(dict(profile=profile, cases=natural['cases'],
            natural_tests=len(natural['tests']), extended_tests=len(extended['tests']),
            native_tool_roundtrip_passed=extended['native_tool_roundtrip_passed'],
            exact_long_output_reference_matched=extended['exact_long_output_reference_matched'],
            long_summary=extended['long_summary']))
        save()
        if profile!='gpfs_b':
            STATE['phase'] = 'draining_completed_profile'
            save()
            run('manage_xolotl_engram_disk_v68.frozen.py', profile, 'drain')
            wait_service(profile, 'stopped', 120)
    STATE.update(phase='passed', finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='requires_inspection', error=type(error).__name__+': '+str(error), finished_unix=time.time())
    save()
    raise
