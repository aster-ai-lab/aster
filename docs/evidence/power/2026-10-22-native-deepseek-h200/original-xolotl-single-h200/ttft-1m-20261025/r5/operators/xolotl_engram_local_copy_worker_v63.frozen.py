"""Stage exact Engram blobs on owned local disk at low priority; never change a live service."""
import concurrent.futures
import hashlib
import json
import os
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'copy-plan.json').read_text())
LOCAL = Path(PLAN['local_root'])
LOCK = threading.RLock()
STATE = dict(schema='aster-xolotl-engram-local-copy-v63', unix=time.time(), phase='preflight',
             gpu_executed=False, live_service_changed=False, files=[], local_root=str(LOCAL))
MIN_FREE = 64 * 1024**3
CHUNK = 4 * 1024**2


def save():
    with LOCK:
        temp = ROOT / 'copy-result.tmp'
        temp.write_text(json.dumps(STATE, indent=2) + '\n')
        temp.replace(ROOT / 'copy-result.json')


def throttle(started, count, rate):
    delay = count / rate - (time.monotonic() - started)
    if delay > 0:
        time.sleep(min(delay, 1))


def copy_one(source):
    path = Path(source['physical_file'])
    if path.resolve() != path or not path.is_file() or path.stat().st_size != source['bytes']:
        raise RuntimeError('exact original Engram blob identity/size differs')
    name = source['expected_sha256'] + '.safetensors'
    temporary = LOCAL / (name + '.partial')
    row = dict(file=name, expected_bytes=source['bytes'], copied_bytes=0, verified_bytes=0, phase='copying')
    with LOCK:
        STATE['files'].append(row)
        save()
    digest = hashlib.sha256()
    start = time.monotonic()
    last_save = start
    flushed = 0
    with path.open('rb', buffering=0) as source_fd, temporary.open('xb', buffering=0) as output:
        while True:
            data = source_fd.read(CHUNK)
            if not data:
                break
            if os.statvfs(LOCAL).f_bavail * os.statvfs(LOCAL).f_frsize < MIN_FREE:
                raise RuntimeError('owned local disk reached its64GiB reserve; keep partial file for inspection')
            digest.update(data)
            written = output.write(data)
            if written != len(data):
                raise RuntimeError('short local Engram blob write')
            row['copied_bytes'] += written
            if row['copied_bytes'] - flushed >= 64 * 1024**2:
                output.flush()
                os.fdatasync(output.fileno())
                os.posix_fadvise(output.fileno(), flushed, row['copied_bytes'] - flushed, os.POSIX_FADV_DONTNEED)
                flushed = row['copied_bytes']
            throttle(start, row['copied_bytes'], PLAN['copy_bytes_per_second_per_file'])
            if time.monotonic() - last_save >= 5:
                with LOCK:
                    save()
                last_save = time.monotonic()
        os.fdatasync(output.fileno())
        os.posix_fadvise(output.fileno(), 0, 0, os.POSIX_FADV_DONTNEED)
    if row['copied_bytes'] != source['bytes'] or digest.hexdigest() != source['expected_sha256']:
        raise RuntimeError('original Engram blob full SHA256 differs; do not use staged bytes')
    row.update(phase='verifying_local_readback', source_sha256=digest.hexdigest(),
               copy_seconds=time.monotonic() - start)
    save()
    digest = hashlib.sha256()
    start = time.monotonic()
    last_save = start
    with temporary.open('rb', buffering=0) as local:
        while True:
            data = local.read(CHUNK)
            if not data:
                break
            digest.update(data)
            row['verified_bytes'] += len(data)
            os.posix_fadvise(local.fileno(), row['verified_bytes'] - len(data), len(data), os.POSIX_FADV_DONTNEED)
            throttle(start, row['verified_bytes'], PLAN['verify_bytes_per_second_per_file'])
            if time.monotonic() - last_save >= 5:
                save()
                last_save = time.monotonic()
    if row['verified_bytes'] != source['bytes'] or digest.hexdigest() != source['expected_sha256']:
        raise RuntimeError('local Engram full readback SHA256 differs; do not use staged bytes')
    destination = LOCAL / name
    if destination.exists():
        raise RuntimeError('exclusive final local Engram blob already exists')
    temporary.rename(destination)
    row.update(phase='passed', local_sha256=digest.hexdigest(), verify_seconds=time.monotonic() - start)
    save()


try:
    os.umask(0o077)
    os.nice(19)
    save()
    if (os.getuid() != 10250 or LOCAL.parent != Path('/var/tmp')
            or LOCAL.parent.resolve() != LOCAL.parent or LOCAL.exists()
            or PLAN['allocation_uid'] != '08538623-2284-45f9-b98a-7733bb70f32d'):
        raise RuntimeError('exclusive optimization-only local staging identity differs')
    mounts = []
    for line in Path('/proc/self/mountinfo').read_text().splitlines():
        before, after = line.split(' - ', 1)
        mount = before.split()[4]
        if str(LOCAL.parent) == mount or str(LOCAL.parent).startswith(mount.rstrip('/') + '/'):
            mounts.append((len(mount), mount, after.split()[0]))
    filesystem = sorted(mounts)[-1]
    if filesystem[2] not in ['overlay', 'xfs', 'ext4']:
        raise RuntimeError('Engram staging requires local disk, not tmpfs or network storage')
    blobs = PLAN['blobs']
    if len(blobs) != 2 or sum(row['bytes'] for row in blobs) != 203073077576:
        raise RuntimeError('exact two original Engram blobs differ')
    available = os.statvfs(LOCAL.parent)
    if available.f_bavail * available.f_frsize < sum(row['bytes'] for row in blobs) + MIN_FREE:
        raise RuntimeError('owned local disk has insufficient space plus64GiB reserve')
    index = Path(PLAN['source_index']).read_bytes()
    expected_names = {row['expected_sha256'] + '.safetensors' for row in blobs}
    if len(index) > 4096 or set(json.loads(index)['weight_map'].values()) != expected_names:
        raise RuntimeError('exact original Engram index refers to different blobs')
    LOCAL.mkdir(mode=0o700)
    STATE.update(phase='copying', filesystem=list(filesystem), nice=19,
                 source_index_sha256=hashlib.sha256(index).hexdigest(),
                 copy_bytes_per_second_per_file=PLAN['copy_bytes_per_second_per_file'],
                 verify_bytes_per_second_per_file=PLAN['verify_bytes_per_second_per_file'])
    save()
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        futures = [executor.submit(copy_one, row) for row in blobs]
        for future in futures:
            future.result()
    with (LOCAL / 'model.safetensors.index.json').open('xb') as output:
        output.write(index)
    STATE.update(phase='passed', total_bytes=sum(row['bytes'] for row in blobs),
                 all_full_sha256_readbacks_passed=True, exact_original_weight_bytes=True,
                 finished_unix=time.time())
    save()
    with (LOCAL / 'complete.json').open('x') as output:
        output.write(json.dumps(STATE, indent=2) + '\n')
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
