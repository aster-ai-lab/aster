"""Finite replacement stable-resource/model qualification; never resubmit an observed resource."""
import json
import subprocess
import sys
import time
from pathlib import Path

B = Path(__file__).resolve().parent
STATE = dict(unix=time.time(), phase='starting', profiles=[], formal_service_unchanged=True,
             ctx1M_configured=True, actual1M_input_tested=False)
DEADLINE = time.monotonic() + 7200


def save():
    (B / 'xolotl-stable-recovery-v58-controller.json').write_text(json.dumps(STATE, indent=2) + '\n')


def command(script, *args):
    result = subprocess.run([sys.executable, '-X', 'utf8', str(B / script), *args],
                            capture_output=True, text=True, timeout=110)
    if result.returncode:
        raise RuntimeError(script + ' failed: ' + result.stderr[-4000:])
    return json.loads(result.stdout)


def pause():
    if time.monotonic() > DEADLINE:
        raise RuntimeError('finite local comparison budget exhausted; inspect exact existing handles')
    time.sleep(10)


try:
    while True:
        resource=command('xolotl_cont8_common_v58.py')
        STATE.update(phase='waiting_existing_resource',resource_phase=resource['phase'],resource_probe=resource.get('proof'))
        save()
        if resource['phase']=='Running' and resource.get('proof')=='passed':break
        if resource['phase'] in ['Succeeded','Failed','Stopped']:raise RuntimeError('exact replacement resource terminal; inspect it without resubmission')
        pause()
    for profile in ['stable']:
        STATE.update(phase='starting_profile', current_profile=profile)
        save()
        if not (B / ('xolotl-stable-recovery-v58-' + profile + '-intent.json')).exists():
            command('launch_xolotl_stable_recovery_v58.frozen.py')
        while True:
            value = command('manage_xolotl_stable_recovery_v58.frozen.py', profile, 'observe')
            q = value.get('service-longctx-v1-result.json', {})
            STATE['service_phase'] = q.get('phase')
            save()
            if q.get('phase') == 'serving':
                break
            if q.get('phase') in ['failed', 'stopped'] or not value['exact_supervisor_alive']:
                raise RuntimeError('exact comparison service terminal before qualification')
            pause()
        if not (B / ('xolotl-stable-recovery-v58-' + profile + '-qualification-intent.json')).exists():
            command('manage_xolotl_stable_recovery_v58.frozen.py', profile, 'launch_client')
        last = None
        while True:
            basic = command('manage_xolotl_stable_recovery_v58.frozen.py', profile, 'observe_client')
            current = (basic['phase'], basic['tests'])
            STATE.update(phase='basic28', client_phase=basic['phase'], client_tests=basic['tests'])
            save()
            if current != last:
                print(json.dumps(dict(profile=profile, stage='basic28', phase=basic['phase'], tests=basic['tests'])), flush=True)
                last = current
            if basic['phase'] == 'passed' and not basic['exact_client_alive']:
                break
            if basic['phase'] == 'failed' or not basic['exact_client_alive']:
                raise RuntimeError('exact basic28 client failed: ' + str(basic.get('error')))
            pause()
        if not (B / ('xolotl-stable-extended-v58-' + profile + '-intent.json')).exists():
            command('manage_xolotl_stable_extended_v58.frozen.py', profile, 'launch')
        last = None
        while True:
            extended = command('manage_xolotl_stable_extended_v58.frozen.py', profile, 'observe')
            current = (extended['phase'], extended['tests'])
            STATE.update(phase='extended', client_phase=extended['phase'], client_tests=extended['tests'])
            save()
            if current != last:
                print(json.dumps(dict(profile=profile, stage='extended', phase=extended['phase'], tests=extended['tests'],
                    tool_roundtrip=extended['native_tool_roundtrip_passed'])), flush=True)
                last = current
            if extended['phase'] == 'passed' and not extended['exact_client_alive']:
                break
            if extended['phase'] == 'failed' or not extended['exact_client_alive']:
                raise RuntimeError('native tool/long-output client failed: ' + str(extended.get('error')))
            pause()
        STATE['profiles'].append(dict(profile=profile, basic=basic, extended=extended))
        save()
        print(json.dumps(dict(profile=profile, phase='passed', short_summary=basic['short_summary'],
            long_summary=extended['long_summary'], native_tool_roundtrip_passed=True,
            exact_output_reference_matched=extended['reference_matched'])), flush=True)
        if not (B/'xolotl-stable-edge-suite-v59-intent.json').exists():
            command('manage_xolotl_stable_edge_suite_v59.frozen.py','launch')
        while True:
            edge=command('manage_xolotl_stable_edge_suite_v59.frozen.py','observe')
            STATE.update(phase='strict132097',edge_phase=edge['phase'],edge_stage=edge.get('current_stage'))
            save()
            if edge['phase']=='passed' and not edge['exact_client_alive']:break
            if edge['phase']=='failed' or not edge['exact_client_alive']:raise RuntimeError('exact replacement132097 gate failed: '+str(edge.get('error')))
            pause()
        STATE['edge_passed']=True
    STATE.update(phase='passed', finished_unix=time.time(), qualified_stable_service_kept=True)
    save()
    print(json.dumps(dict(phase='passed', profiles=len(STATE['profiles']), actual1M_input_tested=False)), flush=True)
except BaseException as error:
    STATE.update(phase='needs_inspection', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
