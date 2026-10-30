"""Read exact real Engram weight/scale rows through GPFS and local disk; CPU/disk only."""
import concurrent.futures
import hashlib
import json
import os
import random
import statistics
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'probe-plan.json').read_text())
STATE = dict(schema='aster-xolotl-engram-storage-probe-v64', unix=time.time(), phase='preflight',
             scope='CPU dispatch plus paired256-byte weight/8-byte scale preads; component probe, not model/API speed.',
             gpu_executed=False, live_service_changed=False, source_cache_evicted=False, cases=[])


def save():
    temp = ROOT / 'probe-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'probe-result.json')


try:
    os.umask(0o077)
    save()
    complete = json.loads((Path(PLAN['local_root']) / 'complete.json').read_text())
    if complete['phase'] != 'passed' or not complete['all_full_sha256_readbacks_passed']:
        raise RuntimeError('exact203GB local Engram full readback gate must pass first')
    tables = []
    for row in PLAN['tables']:
        source = Path(row['physical_file'])
        local = Path(PLAN['local_root']) / (row['expected_sha256'] + '.safetensors')
        if (source.resolve() != source or local.is_symlink() or source.stat().st_size != row['bytes']
                or local.stat().st_size != row['bytes'] or local.stat().st_uid != 10250):
            raise RuntimeError('exact source/local Engram blob size/owner differs')
        with source.open('rb') as a, local.open('rb') as b:
            length_a = int.from_bytes(a.read(8), 'little')
            length_b = int.from_bytes(b.read(8), 'little')
            if length_a != length_b or length_a > 8192:
                raise RuntimeError('bounded exact Engram table header differs')
            header_a = a.read(length_a)
            if header_a != b.read(length_b):
                raise RuntimeError('local original Engram table header bytes differ')
        header = json.loads(header_a)
        weight = header[row['weight_name']]
        scale = header[row['scale_name']]
        if (weight['dtype'] != 'F8_E4M3' or scale['dtype'] != 'F8_E8M0'
                or weight['shape'][1:] != [256] or scale['shape'] != [weight['shape'][0], 8]):
            raise RuntimeError('exact Engram embedding weight/scale geometry differs')
        tables.append(dict(source=str(source), local=str(local), rows=weight['shape'][0],
                           weight_offset=8 + length_a + weight['data_offsets'][0],
                           scale_offset=8 + length_a + scale['data_offsets'][0]))
    if len(tables) != 2:
        raise RuntimeError('exact two Engram embedding tables required')
    for count in [512, 4096]:
        rng = random.Random(20261025 + count)
        samples = [(index, rng.randrange(table['rows'])) for index, table in enumerate(tables) for _ in range(count)]
        wanted = None
        results = []
        # Alternate order; no shared/original table cache is evicted.
        for repeat, order in enumerate([['source', 'local'], ['local', 'source'], ['source', 'local']]):
            for backend in order:
                handles = [os.open(table[backend], os.O_RDONLY) for table in tables]
                try:
                    def read(sample):
                        index, number = sample
                        table = tables[index]
                        w = os.pread(handles[index], 256, table['weight_offset'] + number * 256)
                        s = os.pread(handles[index], 8, table['scale_offset'] + number * 8)
                        if len(w) != 256 or len(s) != 8:
                            raise RuntimeError('short exact Engram row read')
                        return w + s

                    start = time.monotonic()
                    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as executor:
                        rows = list(executor.map(read, samples))
                    seconds = time.monotonic() - start
                finally:
                    for handle in handles:
                        os.close(handle)
                digest = hashlib.sha256(b''.join(rows)).hexdigest()
                wanted = wanted or digest
                if digest != wanted:
                    raise RuntimeError('same real Engram sampled weight/scale bytes differ between backends/repetitions')
                result = dict(rows_per_table=count, backend=backend, repetition=repeat,
                              rows=len(samples), pread_calls=2 * len(samples), bytes=264 * len(samples),
                              seconds=seconds, paired_rows_per_second=len(samples) / seconds,
                              output_sha256=digest)
                results.append(result)
                STATE['cases'].append(result)
                STATE['phase'] = 'measuring'
                save()
        STATE.setdefault('summaries', []).append(dict(rows_per_table=count,
            source_seconds=[row['seconds'] for row in results if row['backend'] == 'source'],
            local_seconds=[row['seconds'] for row in results if row['backend'] == 'local'],
            source_median_seconds=statistics.median(row['seconds'] for row in results if row['backend'] == 'source'),
            local_median_seconds=statistics.median(row['seconds'] for row in results if row['backend'] == 'local'),
            all_sampled_raw_bytes_identical=True, sha256=wanted))
    STATE.update(phase='passed', measured_blocks=12, all_sampled_raw_bytes_identical=True, finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
