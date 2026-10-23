#!/usr/bin/env python3
"""Read-only bounded Power metric sampling while an existing campaign runs."""
import argparse
import importlib.util
import json
import math
from pathlib import Path
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('power_sampler_matrix', HERE / 'matrix-campaign.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
GAUGES = frozenset(('power_active_requests', 'power_requests_waiting', 'power_requests_running'))
SHAPES = {'power_http_admission_wait_seconds_bucket': {'outcome', 'le'},
          'power_http_admission_wait_seconds_count': {'outcome'},
          'power_http_admission_wait_seconds_sum': {'outcome'},
          'power_http_admission_refusals_total': {'reason'}}
OUTCOMES = frozenset(('admitted', 'queue_full', 'timeout', 'cancelled', 'unavailable'))


def select(raw):
    selected = {}
    for line in raw.decode().splitlines():
        fields = line.split()
        if len(fields) != 2:
            continue
        metric = fields[0]
        name, separator, labels = metric.partition('{')
        if name not in GAUGES and name not in SHAPES:
            continue
        value = float(fields[1])
        if not math.isfinite(value) or value < 0 or (not name.endswith('_sum') and (not value.is_integer() or value > 2**53)):
            raise ValueError('invalid closed-series value')
        if separator:
            if not labels.endswith('}'):
                raise ValueError('invalid labels')
            parsed = {}
            for part in labels[:-1].split(','):
                key, equals, encoded = part.partition('=')
                if not equals or key in parsed or len(encoded) < 2 or encoded[0] != '"' or encoded[-1] != '"':
                    raise ValueError('invalid label shape')
                parsed[key] = encoded[1:-1]
            if set(parsed) != SHAPES.get(name):
                raise ValueError('unknown labels')
            for key, label in parsed.items():
                if key in ('outcome', 'reason') and (label not in OUTCOMES or (key == 'reason' and label == 'admitted')):
                    raise ValueError('unknown admission outcome')
                if key == 'le' and label != '+Inf':
                    bound = float(label)
                    if not math.isfinite(bound) or not 0 < bound <= 60:
                        raise ValueError('invalid histogram bound')
        elif name not in GAUGES:
            raise ValueError('admission series lacks labels')
        if metric in selected or len(selected) >= 128:
            raise ValueError('duplicate or unbounded series')
        selected[metric] = value
    if not GAUGES.issubset(selected):
        raise ValueError('missing gauges')
    return {'series': selected, 'admission_series_observed': any(name.partition('{')[0] in SHAPES for name in selected)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--watch-pid', required=True, type=int)
    parser.add_argument('--seconds', type=int, default=5400, choices=range(1, 5401))
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    stat = Path('/proc') / str(args.watch_pid) / 'stat'
    initial = m.start_ticks(m.read_limited(stat, 65536))
    started, deadline, index = time.monotonic(), time.monotonic() + args.seconds, 0
    with args.output.open('x', encoding='utf-8', buffering=1) as output:
        output.write(json.dumps({'kind': 'header', 'watch_pid': args.watch_pid, 'watch_start_ticks': initial,
                                 'started_monotonic': started, 'period_seconds': 1,
                                 'tool_identity': dict(m.TOOL_IDENTITY, sampler_sha256=m.digest(Path(__file__).resolve()))}) + '\n')
        while time.monotonic() < deadline:
            try:
                raw_stat = m.read_limited(stat, 65536)
                if m.start_ticks(raw_stat) != initial or raw_stat.rsplit(b')', 1)[1].split()[0] == b'Z':
                    break
            except FileNotFoundError:
                break
            row = {'kind': 'sample', 'index': index, 'at_monotonic': time.monotonic(), 'arms': {}}
            cycle_started = row['at_monotonic']
            for arm, port in m.POWER_PORTS.items():
                try:
                    row['arms'][arm] = select(m.get('/metrics', port=port, timeout=.75))
                except Exception as exc:
                    row['arms'][arm] = {'observation_error': type(exc).__name__}
            output.write(json.dumps(row, allow_nan=False) + '\n')
            index += 1
            time.sleep(max(0, min(deadline - time.monotonic(), 1 - (time.monotonic() - cycle_started))))
        output.write(json.dumps({'kind': 'end', 'at_monotonic': time.monotonic(), 'samples': index}) + '\n')


if __name__ == '__main__':
    main()
