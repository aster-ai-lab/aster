#!/usr/bin/env python3
"""Summarize preserved rig7 results without making network requests."""
import importlib.util
import json
from pathlib import Path
import statistics

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('formal_summary_matrix', HERE / 'matrix-campaign.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def spread(values):
    return {'median': statistics.median(values), 'min': min(values), 'max': max(values), 'repeats': values}


def main():
    runs = {}
    for arm in ('baseline', 'candidate'):
        for concurrency in (1, 8, 16, 32):
            runs[arm, concurrency] = []
            for repetition in (1, 2, 3):
                path = HERE / 'formal-software-v3' / f'{arm}-c{concurrency}-r{repetition}.json'
                row = json.loads(path.read_text())
                if row['qualification_status'] != 'passed' or row['tool_identity'] != m.TOOL_IDENTITY:
                    raise ValueError('failed result or unfrozen tool identity')
                runs[arm, concurrency].append(row)
    result = {'scope': 'rig7 artifact comparison on one shared NP1 native, three matched repeats per concurrency',
              'tool_identity': m.TOOL_IDENTITY, 'formal_runs': 24, 'passed_runs': 24, 'by_concurrency': {},
              'limitations': [
                  'Deployed baseline source/features are unverified; this is an operational artifact comparison.',
                  'Baseline streaming per-request usage is not authoritative; native aggregate counter deltas supply throughput.',
                  'Buffered JSON TTFT is null. First meaningful output and tool first-output use only successful SSE observations.',
                  'Overload refusals are separate from successful throughput; endpoint fairness and admitted case mix can change under refusal.',
                  'Companion v1 began about 189s after formal start and could not parse candidate histogram max bucket; raw observation failures retained.',
                  'Companion v2 candidate series begin about 326.6s after formal start; no fabricated samples fill either gap.',
                  'Greedy temperature zero is matched; requested seed42 was not forwarded, upstream seed is auto4294967295.'
              ]}
    for concurrency in (1, 8, 16, 32):
        entry = {}
        for arm in ('baseline', 'candidate'):
            rows = runs[arm, concurrency]
            records = [record for row in rows for record in row['records']]
            wall = sum(row['summary']['wall_seconds_including_drain'] for row in rows)
            entry[arm] = {'native_output_tokens_per_second': spread([row['native_output_tokens_per_second'] for row in rows]),
                          'native_predicted_token_delta_total': sum(row['native_predicted_token_delta'] for row in rows),
                          'client_pooled': m.q.summarize(records, wall),
                          'client_p95_ms_by_repeat': {field: spread([row['summary'][field]['p95'] for row in rows])
                                                      for field in ('ttft_ms', 'elapsed_ms', 'first_tool_ms')},
                          'cases': {name: m.q.summarize([record for record in records if record['case'] == name], wall)
                                    for name in rows[0]['case_summaries']}}
        result['by_concurrency'][str(concurrency)] = entry
    c1 = result['by_concurrency']['1']
    gates = {}
    for field in ('ttft_ms', 'elapsed_ms'):
        old = c1['baseline']['client_p95_ms_by_repeat'][field]
        new = c1['candidate']['client_p95_ms_by_repeat'][field]
        paired = [100 * (b / a - 1) for a, b in zip(old['repeats'], new['repeats'])]
        gates[field] = {'median_p95_change_percent': 100 * (new['median'] / old['median'] - 1),
                        'paired_repeat_change_percent': paired,
                        'each_repeat_within_ten_percent': all(value <= 10 for value in paired)}
    gates['unexpected_errors_zero'] = all(sum(value for outcome, value in entry[arm]['client_pooled']['outcomes'].items()
                                                    if outcome not in ('success', 'refused')) == 0
                                          for entry in result['by_concurrency'].values() for arm in ('baseline', 'candidate'))
    result['c1_latency_gate'] = gates
    companion = {}
    for suffix in ('', '-v2'):
        path = HERE / ('formal-software-v3-companion' + suffix + '.jsonl')
        rows = [json.loads(line) for line in path.read_text().splitlines()]
        samples = [row for row in rows if row['kind'] == 'sample']
        arms = {}
        for arm in ('baseline', 'candidate'):
            valid = [row for row in samples if 'series' in row['arms'][arm]]
            arms[arm] = {'valid_samples': len(valid), 'observation_errors': len(samples) - len(valid),
                         'first_valid_monotonic': valid[0]['at_monotonic'] if valid else None,
                         'last_valid_monotonic': valid[-1]['at_monotonic'] if valid else None,
                         'peak_gauges': {name: max(row['arms'][arm]['series'][name] for row in valid)
                                         for name in ('power_active_requests', 'power_requests_waiting', 'power_requests_running')} if valid else None,
                         'admission_series_observed': any(row['arms'][arm].get('admission_series_observed') for row in valid)}
        companion[path.name] = {'header': rows[0], 'arms': arms}
    result['companion_coverage'] = companion
    (HERE / 'formal-software-v3-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'by_concurrency': {c: {arm: {'native_tps': data[arm]['native_output_tokens_per_second'],
                                                   'counts': data[arm]['client_pooled']['outcomes'],
                                                   'pooled_p95_ms': {field: data[arm]['client_pooled'][field]['p95'] for field in ('ttft_ms', 'elapsed_ms', 'first_tool_ms')}}
                                           for arm in ('baseline', 'candidate')} for c, data in result['by_concurrency'].items()},
                      'c1_latency_gate': gates, 'companion_coverage': companion}, indent=2))


if __name__ == '__main__':
    main()
