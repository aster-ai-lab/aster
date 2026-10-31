#!/usr/bin/env python3
"""Validate the actual Power executable and auth, before inference activation."""

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

import service


def main():
    if (service.ROOT/'run/active-profile.json').exists():
        raise RuntimeError('frontend smoke must run before an inference profile is active')
    subprocess.run([sys.executable, __file__.replace('frontend_smoke.py', 'service.py'), 'configure'], check=True)
    environment = dict(os.environ)
    environment['ASTER_POWER_HOME'] = str(service.PRIVATE/'data')
    with (service.ROOT/'logs/power-frontend-smoke.log').open('ab') as log:
        process = subprocess.Popen(
            [str(service.ROOT/'bin/aster-power'), 'serve', '--config', str(service.PRIVATE/'power.acl'),
             '--host', '127.0.0.1', '--port', '30003'],
            env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            service.wait_health(process, 30003, 30)
            try:
                urllib.request.urlopen('http://127.0.0.1:30003/v1/models', timeout=5)
                raise RuntimeError('unauthenticated models request was accepted')
            except urllib.error.HTTPError as error:
                if error.code != 401:
                    raise
            request = urllib.request.Request(
                'http://127.0.0.1:30003/v1/models',
                headers={'Authorization': 'Bearer '+(service.PRIVATE/'api.key').read_text().strip()})
            with urllib.request.urlopen(request, timeout=5) as response:
                models = json.load(response)
            if service.MODEL_NAME not in [model['id'] for model in models['data']]:
                raise RuntimeError('configured remote alias is missing')
            result = {'authenticated_models': True, 'unauthenticated_status': 401,
                      'alias': service.MODEL_NAME, 'inference_tested': False, 'unix': time.time()}
            (service.ROOT/'run/power-frontend-smoke.json').write_text(json.dumps(result, indent=2))
            print(json.dumps(result), flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == '__main__':
    main()
