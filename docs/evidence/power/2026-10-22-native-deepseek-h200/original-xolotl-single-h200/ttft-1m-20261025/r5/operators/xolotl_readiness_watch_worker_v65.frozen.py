"""Finite server-side observation only; no restart, resource creation or Windows forwarding."""
import json
import shlex
import subprocess
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'watch-plan.json').read_text())
STATE = dict(schema='aster-xolotl-readiness-watch-v65', unix=time.time(), phase='starting',
             allocation_uid=PLAN['allocation_uid'], observation_only=True, failures=[])


def save():
    temp = ROOT / 'watch-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'watch-result.json')


def cli(kind, name):
    args = ['brainctl', 'get', kind, name, '-n', 'ailab-hrprdduanyan', '-o', 'json']
    result = subprocess.run(['bash', '-lc', '. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec ' + shlex.join(args)],
                            capture_output=True, text=True, timeout=15)
    if result.returncode:
        raise RuntimeError('bounded readiness ' + kind + ' observation failed')
    if len(result.stdout) > 1048576:
        raise RuntimeError('bounded readiness metadata exceeded')
    return json.loads(result.stdout)


save()
while time.time() < PLAN['lease_ends_unix'] + 30:
    key = ''
    try:
        job = cli('rjob', PLAN['allocation_job'])
        if job['metadata']['uid'] != PLAN['allocation_uid']:
            raise RuntimeError('readiness allocation UID changed; no mutation permitted')
        STATE.update(unix=time.time(), job_phase=job['status']['phase'])
        if job['status']['phase'] in ['Succeeded', 'Failed', 'Stopped']:
            STATE.update(phase='resource_terminal', terminal_status=job['status'], finished_unix=time.time())
            save()
            break
        service = cli('service', PLAN['service_name'])
        if (service['metadata']['uid'] != PLAN['service_uid']
                or service['metadata']['annotations']['aster.serving/allocation-uid'] != PLAN['allocation_uid']):
            STATE.update(phase='superseded', finished_unix=time.time())
            save()
            break
        key = (Path(PLAN['service_root']) / 'credentials/api-key.txt').read_text().strip()
        request = urllib.request.Request(PLAN['health_url'], headers={'Authorization': 'Bearer ' + key})
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=5) as response:
            data = response.read(65537)
            if len(data) > 65536:
                raise RuntimeError('bounded readiness health response exceeded')
            health = json.loads(data)
            if response.status != 200 or not health.get('loaded') or health.get('max_context') != 1048576:
                raise RuntimeError('current bound API is not loaded at1M')
        STATE.update(phase='ready', last_ready_unix=time.time(), health_status=200,
                     scope='jump host direct to the owned VIP; no inference request')
    except Exception as error:
        message = type(error).__name__ + ': ' + str(error)
        if key:
            message = message.replace(key, '[REDACTED]')
        STATE.update(phase='observation_failed', unix=time.time(), error=message)
        STATE['failures'] = (STATE['failures'] + [dict(unix=time.time(), error=message)])[-16:]
    save()
    time.sleep(min(60, max(0.1, PLAN['lease_ends_unix'] + 30 - time.time())))
else:
    STATE.update(phase='lease_end_observed', finished_unix=time.time())
    save()
