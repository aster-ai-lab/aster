"""Qualify and run one native Power service inside an already owned GPU lease.

Settings are frozen ACL inputs. Credentials are newly generated private lease
artifacts and never appear in inputs, logs, commands or evidence receipts.
"""
import gzip
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import time
import urllib.error
import urllib.request
import uuid


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1024**2):
            result.update(chunk)
    return result.hexdigest()


def unpack(root, work, name, artifact):
    archive = root / (name + '.gz')
    if (not 0 < artifact['archive_bytes'] <= 256 * 1024**2
            or archive.stat().st_size != artifact['archive_bytes']
            or digest(archive) != artifact['archive_sha256']
            or not 0 < artifact['binary_bytes'] <= 512 * 1024**2):
        raise RuntimeError('native executable archive identity/size changed')
    target = work / name
    count = 0
    with gzip.open(archive, 'rb') as source, target.open('xb') as output:
        while chunk := source.read(1024**2):
            count += len(chunk)
            if count > artifact['binary_bytes']:
                raise RuntimeError('native executable expansion exceeds its pinned size')
            output.write(chunk)
    if count != artifact['binary_bytes'] or digest(target) != artifact['binary_sha256']:
        raise RuntimeError('expanded executable differs from its frozen build')
    target.chmod(0o700)
    return target


def validate_admissions(admissions, identities):
    if len(admissions) != 4 or len(identities) != 8 or len(set(identities)) != 8:
        raise RuntimeError('service requires four complete non-overlapping two-H200 replicas')
    for group, devices in enumerate(admissions):
        if len(devices) != 2:
            raise RuntimeError('native service admitted an unexpected replica width')
        for local, device in enumerate(devices):
            ordinal = group * 2 + local
            if (device['ordinal'] != ordinal or device['capability'] != [9, 0]
                    or 'GPU-' + str(uuid.UUID(bytes=bytes(device['uuid']))) != identities[ordinal]
                    or device['residentUpperBytes'] + device['stateAndDriverReserve']
                    > device['freeBytesAtLoad']):
                raise RuntimeError('native admission differs from its exact physical H200 assignment')


def main():
    root = Path(__file__).absolute().parent
    if root.resolve() != root or root.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('native deployment requires its exact owned stage')
    os.umask(0o077)
    result = dict(schema='aster-native-power-service-v1', phase='preflight', unix=time.time(),
                  final_user_delivery_claimed=False, quality_or_performance_qualification_claimed=False)
    child = None
    profile = None
    server = None

    def publish():
        temporary = root / 'service-result.tmp'
        temporary.write_text(json.dumps(result, indent=2) + '\n')
        temporary.replace(root / 'service-result.json')

    def interrupted(number, _frame):
        raise RuntimeError('owned service operator interrupted by signal ' + str(number))

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    try:
        inputs_path = root / 'deploy-inputs.json'
        if inputs_path.stat().st_size > 65536:
            raise RuntimeError('native deployment manifest exceeds its read bound')
        inputs = json.loads(inputs_path.read_text())
        if (inputs['schema'] != 'aster-native-power-deploy-inputs-v1'
                or inputs['worker_sha256'] != digest(Path(__file__))):
            raise RuntimeError('native service input/source identity changed')
        profile = inputs.get('profile')
        if profile not in (None, 'nsys-interactive-v1'):
            raise RuntimeError('native service profile requires nsys-interactive-v1 or absent')
        expected_files = {'serve-xq-native.gz', 'native-controls.gz', 'power.acl', 'native.acl'}
        if profile:
            expected_files |= {'profile_target.py', 'native_profile.py'}
        if {value['file'] for value in inputs['files']} != expected_files:
            raise RuntimeError('native service requires its exact frozen artifact set')
        for value in inputs['files']:
            path = root / value['file']
            if (path.resolve() != path or not path.is_file() or path.is_symlink()
                    or path.stat().st_uid != 10250 or path.stat().st_size != value['bytes']
                    or digest(path) != value['sha256']):
                raise RuntimeError('frozen service artifact identity changed')
        proof_path = Path(inputs['resource_proof'])
        if proof_path.stat().st_size > 262144:
            raise RuntimeError('resource proof exceeds its read bound')
        proof = json.loads(proof_path.read_text())
        if (proof['phase'] != 'passed' or proof['hostname'] != socket.gethostname()
                or proof['expectedGpus'] != 8 or proof['leaseEndsUnix'] - time.time() < 1800):
            raise RuntimeError('service requires at least thirty minutes in the verified owned eight-H200 lease')
        identities = [gpu['uuid'] for gpu in proof['gpus']]
        actual = subprocess.run(['nvidia-smi', '--query-gpu=uuid', '--format=csv,noheader,nounits'],
                                capture_output=True, text=True, timeout=15, check=True).stdout.splitlines()
        if len(actual) != 8 or set(actual) != set(identities):
            raise RuntimeError('visible physical GPU assignment differs from its resource proof')
        busy = subprocess.run(['nvidia-smi', '--query-compute-apps=pid,gpu_uuid', '--format=csv,noheader,nounits'],
                              capture_output=True, text=True, timeout=15, check=True).stdout
        if busy.strip():
            raise RuntimeError('owned allocation already has a CUDA workload')
        work = Path('/tmp') / root.name
        work.mkdir(mode=0o700, exist_ok=False)
        (work / 'power-home').mkdir(mode=0o700)
        server = unpack(root, work, 'serve-xq-native', inputs['server_binary'])
        controls = unpack(root, work, 'native-controls', inputs['controls_binary'])
        environment = dict(os.environ)
        paths = [value for value in environment.get('LD_LIBRARY_PATH', '').split(':') if value]
        if any('stubs' in Path(value).parts for value in paths):
            raise RuntimeError('CUDA linker stubs cannot execute the native service')
        environment.update(LD_LIBRARY_PATH=':'.join(['/usr/local/cuda/lib64'] + paths),
                           CUDA_VISIBLE_DEVICES=','.join(identities), ASTER_POWER_HOME=str(work / 'power-home'))
        # Existing ASTER_POWER_* overrides must not silently change frozen ACL.
        forbidden = [name for name in environment if name.startswith('ASTER_POWER_') and name != 'ASTER_POWER_HOME']
        if forbidden:
            raise RuntimeError('unexpected Power environment overrides are present')
        with (root / 'controls.stdout.log').open('x') as stdout, (root / 'controls.stderr.log').open('x') as stderr:
            checked = subprocess.run([str(controls), '--test-threads=1'], stdout=stdout,
                                     stderr=stderr, env=environment, timeout=120)
        output = (root / 'controls.stdout.log').read_text()
        expected = 'test result: ok. ' + str(inputs['expected_controls']) + ' passed; 0 failed; ' + str(inputs['expected_ignored']) + ' ignored;'
        result['controls_exit'] = checked.returncode
        if checked.returncode or expected not in output:
            raise RuntimeError('native serving controls failed; retain and isolate distinct failures')
        result['controls_passed'] = inputs['expected_controls']
        auth = root / 'auth'
        auth.mkdir(mode=0o700)
        key = 'sk-aster-' + secrets.token_urlsafe(32)
        starts = int(time.time())
        expires = min(starts + 7200, int(proof['leaseEndsUnix']) - 60)
        lease = ('schema = "aster.deepseek.credential-lease.v1"\napi_key_sha256 = "'
                 + hashlib.sha256(key.encode()).hexdigest() + '"\nstarts_unix = ' + str(starts)
                 + '\nexpires_unix = ' + str(expires) + '\n')
        (auth / 'credential.acl').write_text(lease)
        with (auth / 'api-key.txt').open('x') as secret:
            secret.write(key + '\n')
        result.update(phase='loading_native_service', credential_starts_unix=starts,
                      credential_expires_unix=expires, source_archive_sha256=inputs['source_archive_sha256'],
                      server_binary_sha256=inputs['server_binary']['binary_sha256'])
        publish()
        if profile:
            from native_profile import await_target
            session = root.name
            command = ['/usr/local/cuda/bin/nsys', 'launch', '--session-new=' + session,
                       '--trace=cuda', '--wait=primary', '--show-output=true', '/usr/bin/python3',
                       str(root / 'profile_target.py'), str(root), str(server),
                       str(root / 'power.acl'), str(root / 'native.acl')]
            with (root / 'nsys-launch.stdout.log').open('x') as stdout, (root / 'nsys-launch.stderr.log').open('x') as stderr:
                child = subprocess.Popen(command, stdout=stdout, stderr=stderr, env=environment)
            result['profiler'] = dict(profile=profile, session=session, pid=child.pid)
            publish()
            result['native_process'] = await_target(root, server, child)
        else:
            with (root / 'native.stdout.log').open('x') as stdout, (root / 'native.stderr.log').open('x') as stderr:
                child = subprocess.Popen([str(server), str(root / 'power.acl'), str(root / 'native.acl')],
                                         stdout=stdout, stderr=stderr, env=environment)
            ticks = int(Path('/proc/' + str(child.pid) + '/stat').read_text().split(') ', 1)[1].split()[19])
            result['native_process'] = dict(pid=child.pid, start_ticks=ticks, uid=os.getuid())
        publish()
        load_deadline = min(time.time() + 1800, proof['leaseEndsUnix'] - 180)
        ready = None
        while time.time() < load_deadline:
            if child.poll() is not None:
                raise RuntimeError('native service exited during startup; retain exact stderr')
            path = root / 'native.stdout.log'
            if path.stat().st_size > 4 * 1024**2:
                raise RuntimeError('native service startup log exceeds its receipt bound')
            for line in path.read_text().splitlines():
                event = json.loads(line)
                if event.get('phase') == 'native_replicas_loaded':
                    ready = event
            if ready:
                validate_admissions(ready['admissions'], identities)
                result.update(phase='waiting_http_listener', native_readiness=ready)
                publish()
                break
            time.sleep(2)
        if not ready:
            raise TimeoutError('native service exceeded its frozen load deadline')

        def request(token):
            headers = {'Authorization': 'Bearer ' + token} if token else {}
            query = urllib.request.Request('http://127.0.0.1:30003/v1/models', headers=headers)
            try:
                with urllib.request.urlopen(query, timeout=5) as response:
                    payload = response.read(65537)
                    if len(payload) > 65536:
                        raise RuntimeError('models response exceeds its byte bound')
                    return response.status, json.loads(payload)
            except urllib.error.HTTPError as error:
                return error.code, None
        listener_deadline = min(time.time() + 60, proof['leaseEndsUnix'] - 60)
        while True:
            try:
                status, models = request(key)
                break
            except urllib.error.URLError:
                if time.time() >= listener_deadline or child.poll() is not None:
                    raise
                time.sleep(1)
        anonymous, _ = request(None)
        wrong, _ = request('invalid-native-probe-key')
        if status != 200 or anonymous != 401 or wrong != 401:
            raise RuntimeError('native model discovery or authentication failed')
        if [value['id'] for value in models['data']] != ['deepseek-v4.1-flash-1017-xq']:
            raise RuntimeError('native registry exposed an unexpected model set')
        result.update(phase='service_listener_verified', listener_unix=time.time(),
                      loopback_base_url='http://127.0.0.1:30003/v1',
                      authentication_codes=dict(valid=status, anonymous=anonymous, wrong=wrong), models=models)
        publish()
        while time.time() < proof['leaseEndsUnix'] - 60:
            if child.poll() is not None:
                raise RuntimeError('native service exited before its owned lease deadline')
            time.sleep(2)
        result.update(phase='lease_ended', completed_unix=time.time())
        publish()
    except Exception as error:
        result.update(phase='failed', error_type=type(error).__name__, error=str(error), completed_unix=time.time())
        publish()
        raise
    finally:
        try:
            if profile and server is not None:
                from native_profile import stop_target
                stop_target(root, server)
        finally:
            if child is not None:
                if profile:
                    try:
                        child.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        pass
                if child.poll() is None:
                    child.kill()
                child.wait()
            print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
