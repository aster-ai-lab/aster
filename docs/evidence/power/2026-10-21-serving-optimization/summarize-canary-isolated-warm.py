#!/usr/bin/env python3
"""Read-only analysis of the completed isolated candidate-only production-GPU7 cohort."""
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('isolated_canary_summary_eight', HERE / 'eight-card-qualification.py')
e = importlib.util.module_from_spec(spec)
spec.loader.exec_module(e)
q = e.q
ROOT = e.m.PRIVATE
TAG = 'canary-v4-isolated-warm'


def load(path):
    return json.loads(path.read_text(encoding='utf-8'))


def spread(values):
    return {'median': statistics.median(values), 'min': min(values), 'max': max(values), 'samples': len(values)}


def gpu_memory(value):
    if isinstance(value, dict):
        if 'used_mib' in value and 'total_mib' in value:
            yield (value['used_mib'], value['total_mib'])
        for child in value.values():
            yield from gpu_memory(child)
    elif isinstance(value, list):
        for child in value:
            yield from gpu_memory(child)


def main():
    driver = load(e.PRIVATE / (TAG + '-driver-full.json'))
    assert driver['status'] == 'passed' and len(driver['cells']) == 12
    assert driver['driver_sha256'] == '6bc7ad205d4c27d654553404442fb6d74d4c99577e75ff238d854bea84eb8627'
    assert load(ROOT / (TAG + '-functional.json'))['status'] == 'passed'
    assert load(ROOT / (TAG + '-preparation.json'))['status'] == 'prepared'
    cohorts, counts, identities, memories = {}, {}, [], []
    files = []
    for concurrency in (1, 8, 16, 32):
        rows, rates, latencies = [], [], {name: [] for name in ('ttft_ms', 'elapsed_ms', 'first_tool_ms')}
        for repeat in (1, 2, 3):
            path = e.PRIVATE / (TAG + '-full') / f'candidate-c{concurrency}-r{repeat}.json'
            value = load(path)
            assert value['qualification_status'] == 'passed' and value['monitor_terminated'] and not value['monitor_errors']
            successes = [row for row in value['records'] if row['outcome'] == 'success']
            assert successes and all(q.valid_usage(row['usage']) is not None and row['usage_source'] == 'upstream-usage'
                                     and row['wire_usage'] == row['usage'] and not row['wire_usage_mismatch']
                                     and row['finish_observed'] and row['terminal_observed']
                                     and row['quality_source'] in ('expected-text', 'expected-tool') for row in successes)
            before = value['before_measurement']['native_counters']['7']
            after = value['after_measurement']['native_counters']['7']
            delta = after['llamacpp:tokens_predicted_total'] - before['llamacpp:tokens_predicted_total']
            assert delta == sum(row['usage']['completion_tokens'] for row in successes)
            summary = value['summary']
            assert summary['wire_usage_mismatches'] == 0 and summary['quality_verified_successes'] == len(successes)
            assert all(row['outcome'] in ('success', 'refused') for row in value['records'])
            assert all(row['http_status'] == 429 for row in value['records'] if row['outcome'] == 'refused')
            for key, count in summary['outcomes'].items():
                counts[key] = counts.get(key, 0) + count
            rows.append({'repeat': repeat, 'outcomes': summary['outcomes'],
                         'native_actual_completion_tokens': delta,
                         'native_actual_tokens_per_second': value['native_output_tokens_per_second'],
                         'own_success_requests_per_second': summary['successful_requests_per_second'],
                         'wall_seconds_including_drain': summary['wall_seconds_including_drain']})
            rates.append(value['native_output_tokens_per_second'])
            for key in latencies:
                latencies[key] += [row[key] for row in successes if row[key] is not None]
            identities.append(value['process_identity'])
            memories += list(gpu_memory(value['gpu_samples']))
            files.append({'file': str(path.relative_to(ROOT)), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        cohorts[concurrency] = {'repetitions': rows, 'native_actual_tokens_per_second': spread(rates),
                                'pooled_success_latency_ms': {key: {'samples': len(values),
                                    'p50': q.percentile(values, .5), 'p95': q.percentile(values, .95),
                                    'p99': q.percentile(values, .99)} for key, values in latencies.items()}}
    assert all(identity == identities[0] for identity in identities)
    result = {'status': 'passed', 'scope': 'candidate-only isolated production native GPU7 through private Gateway/Power wrappers',
              'not_claimed': ['actual public8080 capacity', 'eight-card throughput', 'matched before-after speedup',
                              'each HTTP429 has an exported typed admission code'],
              'phase_elapsed_seconds': driver['elapsed_seconds'], 'driver_sha256': driver['driver_sha256'],
              'tool_identity': driver['tool_identity'], 'process_identity': identities[0],
              'per_gpu_context': {'gpu': 7, 'slots': 2, 'context_per_sequence': 32768},
              'cohorts': cohorts, 'outcomes': counts, 'wire_usage_mismatches': 0,
              'all_measured_success_native_completions_reconciled': True,
              'gpu_vram_peak_mib': max(used for used, _ in memories),
              'gpu_vram_peak_fraction': max(used / total for used, total in memories),
              'source_files': files,
              'refusal_note': 'Frozen client exports HTTP429 status only. Typed queue-full overload was proved separately in the strict isolated functional suite.',
              'latency_note': 'Descriptive pooled successful request samples with different admitted workload mixes; no causal decode-kernel acceleration claim.'}
    destination = ROOT / (TAG + '-summary.json')
    with destination.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
