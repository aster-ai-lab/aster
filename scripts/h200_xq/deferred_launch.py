#!/usr/bin/env python3
"""Reserve eight H200s only after all model parts are ready on shared storage."""

import fcntl
import json
import subprocess
import time
import uuid

from service import STAGING
from remote_install import run_preflight

NAMESPACE = 'ailab-hrprdduanyan'


def write_state(value):
    destination = STAGING/'deployment-state.json'
    temporary = destination.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2))
    temporary.replace(destination)


def submission_command(name):
    return ['rjob', 'submit', '--namespace', NAMESPACE, '--name', name,
               '--task-type=normal', '--gpu=8', '--cpu=160', '--memory=1802240',
               '--charged-group=hrprdaster_gpu_pool', '--positive-tags', 'H200', 'feature/gpfs=yes',
               '--mount', 'gpfs://gpfs1/yangchao:/mnt/shared-storage-user/yangchao',
               'gpfs://gpfs2/gpfs2-shared-public:/mnt/shared-storage-gpfs2/gpfs2-shared-public',
               '--store-host-nvme=true', '--private-machine=group', '--preemptible=no',
               '--image=registry.h.pjlab.org.cn/ailab-safeag/zoubo:v2.2', '--restart-policy=never',
               '--', 'bash', '-lc',
               f'python3 -u {STAGING}/remote_install.py >> {STAGING}/logs/{name}.log 2>&1']


def main():
    with (STAGING/'deferred-launch.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        run_preflight()
        launch_if_ready()


def launch_if_ready():
    if (STAGING/'deployment-state.json').exists():
        raise RuntimeError('a deployment submission record already exists; inspect it before retrying')
    parts = [STAGING/f'assets/v3/V41-XQ-1017-v3.gguf.part-{index:02d}-of-16' for index in range(1, 17)]
    while not all(path.is_file() for path in parts):
        print('WAITING_WITHOUT_GPU_RESERVATION', sum(path.is_file() for path in parts), 16, time.time(), flush=True)
        time.sleep(30)
    # Staging may have changed during the long download. Refuse GPU reservation
    # until the prepared software and auxiliary files pass again.
    run_preflight()
    name = 'aster-power-xq-h200-final-'+uuid.uuid4().hex[:12]
    record = {'phase': 'submitting', 'showname': name, 'created_unix': time.time()}
    write_state(record)
    command = submission_command(name)
    submitted = subprocess.run(command, capture_output=True, text=True)
    # Resolve the server-generated metadata name by exact annotation equality.
    # Do not guess it from CLI prose, or submit a duplicate after an ambiguous response.
    listed = subprocess.run(['brainctl', 'get', 'rjobs', '-n', NAMESPACE, '-o', 'json'],
                            check=True, capture_output=True, text=True)
    jobs = json.loads(listed.stdout)['items']
    matches = [job for job in jobs if job['metadata'].get('annotations', {}).get(
        'kubebrain.brainpp.cn/showname') == name]
    if len(matches) != 1:
        record.update({'phase': 'submission_needs_inspection', 'matching_jobs': len(matches),
                       'cli_exit_code': submitted.returncode})
        write_state(record)
        raise RuntimeError('deployment submission did not resolve to exactly one owned RJob')
    record.update({'phase': 'submitted', 'rjob': matches[0]['metadata']['name'],
                   'cli_exit_code': submitted.returncode})
    write_state(record)
    print('H200_DEPLOYMENT_SUBMITTED', json.dumps(record), flush=True)


if __name__ == '__main__':
    main()
