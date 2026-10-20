"""Independent T01 contract checks, written before reading the agent submission."""

import argparse
import ast
import csv
import hashlib
import itertools
import json
import shutil
import subprocess
import sys
from collections import Counter, defaultdict
from datetime import date
from decimal import Decimal as D, ROUND_HALF_UP
from pathlib import Path

MARKETS = ['Africa', 'Asia_Pacific', 'Europe', 'LATAM', 'USCA']


def csv_rows(path):
    with path.open(encoding='utf-8-sig', newline='') as f:
        return list(csv.DictReader(f))


def sha_files(folder):
    return {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(folder.iterdir()) if p.is_file()}


def fixed(value, places):
    return format(value.quantize(D(1).scaleb(-places), rounding=ROUND_HALF_UP), f'.{places}f')


def ratio(numerator, denominator):
    return int((D(numerator) * 10000 / denominator).quantize(D(1), rounding=ROUND_HALF_UP)) if denominator else None


def metrics(rows):
    active = [r for r in rows if r.get('active', True)]
    sums = {key: sum(r[key] for r in active)
            for key in ['quantity', 'sales_milli', 'profit_milli', 'shipping_milli']}
    return dict(row_count=len(active), **sums,
                contribution_milli=sums['profit_milli'] - sums['shipping_milli'],
                profit_margin_bp=ratio(sums['profit_milli'], sums['sales_milli']),
                contribution_margin_bp=ratio(sums['profit_milli'] - sums['shipping_milli'], sums['sales_milli']),
                high_discount_count=sum(r['discount_ppm'] > 200000 for r in active),
                negative_contribution_count=sum(r['profit_milli'] < r['shipping_milli'] for r in active))


def optimum(candidates, budget):
    groups = defaultdict(list)
    for candidate in candidates:
        groups[candidate['row_key']].append(candidate)
    states = {0: (0, [])}
    for key in sorted(groups):
        updated = dict(states)
        for cost, (gain, ids) in states.items():
            for candidate in groups[key]:
                next_cost = cost + candidate['cost_units']
                if next_cost > budget:
                    continue
                option = (gain + candidate['gain_milli'], sorted(ids + [candidate['action_id']]))
                old = updated.get(next_cost)
                if old is None or (-option[0], option[1]) < (-old[0], old[1]):
                    updated[next_cost] = option
        states = updated
    cost, (gain, ids) = min(states.items(), key=lambda item: (-item[1][0], item[0], item[1][1]))
    if len(candidates) <= 12:
        exhaustive = []
        for mask in itertools.product([False, True], repeat=len(candidates)):
            picked = [c for c, include in zip(candidates, mask) if include]
            spent = sum(c['cost_units'] for c in picked)
            if spent <= budget and len({c['row_key'] for c in picked}) == len(picked):
                exhaustive.append((-sum(c['gain_milli'] for c in picked), spent,
                                   sorted(c['action_id'] for c in picked)))
        assert (-gain, cost, ids) == min(exhaustive), 'Oracle DP disagrees with exhaustive enumeration'
    return cost, gain, ids


def oracle(folder):
    products = {r['Product ID']: r for r in csv_rows(folder / 'product_info.csv')}
    normalized = []
    for market in MARKETS:
        filename = market + '_orders.csv'
        for number, r in enumerate(csv_rows(folder / filename), 1):
            product = products[r['Product ID']]
            row = dict(row_key=f'{filename}:{number:06}', source_file=filename, source_row=number,
                       order_id=r['Order ID'], product_id=r['Product ID'], market=market,
                       category=product['Category'], sub_category=product['Sub-Category'],
                       segment=r['Segment'], ship_mode=r['Ship Mode'], priority=r['Order Priority'],
                       order_date=r['Order Date'], ship_date=r['Ship Date'],
                       delivery_days=(date.fromisoformat(r['Ship Date']) - date.fromisoformat(r['Order Date'])).days,
                       quantity=int(r['Quantity']), sales_milli=int(D(r['Sales']) * 1000),
                       profit_milli=int(D(r['Profit']) * 1000), shipping_milli=int(D(r['Shipping Cost']) * 1000),
                       discount_ppm=int(D(r['Discount']) * 1000000))
            normalized.append(row)
    normalized.sort(key=lambda r: r['row_key'])
    effective = {r['row_key']: dict(r, active=True) for r in normalized}
    events = [json.loads(line) for line in (folder / 'adjustments.jsonl').read_text().splitlines() if line.strip()]
    audit, unique, seen = [], [], set()
    for number, event in enumerate(events, 1):
        item = dict(input_line=number, event_id=event['event_id'], row_key=event['row_key'], status='duplicate_event')
        audit.append(item)
        if event['event_id'] not in seen:
            seen.add(event['event_id'])
            unique.append((event, item))
    for event, item in sorted(unique, key=lambda pair: (pair[0]['timestamp'], pair[0]['event_id'])):
        row, kind, amount = effective.get(event['row_key']), event['type'], event['amount_milli']
        if row is None:
            item['status'] = 'unknown_row'
        elif kind not in ['cancel', 'refund', 'shipping_credit']:
            item['status'] = 'unknown_type'
        elif not row['active']:
            item['status'] = 'order_cancelled'
        elif (type(amount) is not int or
              (kind == 'cancel' and amount != 0) or
              (kind != 'cancel' and not 0 < amount <= row['sales_milli' if kind == 'refund' else 'shipping_milli'])):
            item['status'] = 'invalid_amount'
        else:
            item['status'] = 'applied'
            if kind == 'cancel':
                row.update(active=False, sales_milli=0, profit_milli=0, shipping_milli=0)
            elif kind == 'refund':
                row['sales_milli'] -= amount
                row['profit_milli'] -= amount
            else:
                row['shipping_milli'] -= amount
    effective_list = list(effective.values())
    aggregates = {}
    for name, fields in [('market', ['market']), ('market_category', ['market', 'category']),
                         ('segment', ['segment']), ('ship_mode', ['ship_mode']), ('market_month', ['market', 'month'])]:
        groups = defaultdict(list)
        for r in effective_list:
            key = tuple(r['order_date'][:7] if field == 'month' else r[field] for field in fields)
            groups[key].append(r)
        aggregates[name] = [dict(zip(fields, key), **metrics(groups[key])) for key in sorted(groups)]
    summary = dict(metrics(effective_list), input_row_count=len(normalized),
                   unique_order_id_count=len({r['order_id'] for r in normalized}),
                   duplicate_order_id_extra_rows=len(normalized) - len({r['order_id'] for r in normalized}),
                   cancelled_row_count=sum(not r['active'] for r in effective_list),
                   event_status_counts=dict(Counter(a['status'] for a in audit)))
    reconciliation = []
    for source, field, group_key in [('segment_performance.csv', 'Segment', 'segment'),
                                     ('shipping_by_mode.csv', 'Ship_Mode', 'ship_mode')]:
        for reference in csv_rows(folder / source):
            group = reference[field]
            rows = [r for r in normalized if r[group_key] == group]
            n = len(rows)
            sales, profit, shipping = [sum(r[k] for r in rows) for k in ['sales_milli', 'profit_milli', 'shipping_milli']]
            if group_key == 'segment':
                values = {'Order_Count': (D(n), 0), 'Total_Sales': (D(sales) / 1000, 2),
                          'Total_Profit': (D(profit) / 1000, 2),
                          'Profit_Margin': (D(profit) * 100 / sales if sales else D(0), 2),
                          'Avg_Discount': (D(sum(r['discount_ppm'] for r in rows)) / (n * 1000000) if n else D(0), 4)}
            else:
                values = {'Order_Count': (D(n), 0), 'Total_Shipping_Cost': (D(shipping) / 1000, 2),
                          'Avg_Shipping_Cost': (D(shipping) / (n * 1000) if n else D(0), 2),
                          'Avg_Delivery_Days': (D(sum(r['delivery_days'] for r in rows)) / n if n else D(0), 2)}
            for key, (value, places) in values.items():
                actual, ref = fixed(value, places), fixed(D(reference[key]), places)
                reconciliation.append(dict(source_file=source, group=group, field=key, actual=actual,
                                           reference=ref, match=actual == ref))
    reconciliation.sort(key=lambda r: (r['source_file'], r['group'], r['field']))
    policy = json.loads((folder / 'policy.json').read_text())
    candidates = []
    for row in effective_list:
        if not row['active']:
            continue
        possible = []
        if row['ship_mode'] != 'Standard Class' and row['priority'] in policy['shipping_priorities']:
            possible.append(('shipping_switch', row['shipping_milli'] * policy['shipping_saving_bp'] // 10000,
                             policy['shipping_cost_by_market'][row['market']]))
        if row['discount_ppm'] > policy['discount_threshold_ppm'] and row['profit_milli'] < row['shipping_milli']:
            possible.append(('discount_review', row['sales_milli'] * policy['discount_recovery_bp'] // 10000,
                             policy['discount_cost_by_market'][row['market']]))
        for kind, gain, cost in possible:
            if gain > 0:
                candidates.append(dict(action_id=row['row_key'] + '|' + kind, row_key=row['row_key'],
                                       type=kind, cost_units=cost, gain_milli=gain))
    candidates.sort(key=lambda r: r['action_id'])
    spent, gain, ids = optimum(candidates, policy['budget_units'])
    return dict(schema_version='107-v1', normalized_rows=normalized, event_audit=audit,
                effective_rows=effective_list, aggregates=aggregates, reconciliation=reconciliation, summary=summary,
                action_candidates=candidates, selected_actions=[c for c in candidates if c['action_id'] in set(ids)],
                optimization=dict(budget_units=policy['budget_units'], spent_units=spent, gain_milli=gain,
                                  selected_action_ids=ids, projected_contribution_milli=summary['contribution_milli'] + gain),
                input_sha256=sha_files(folder))


def mismatch(actual, expected, path='$'):
    if type(actual) is not type(expected):
        return f'{path}: type {type(actual).__name__}, expected {type(expected).__name__}'
    if isinstance(expected, dict):
        if actual.keys() != expected.keys():
            return f'{path}: keys mismatch actual={sorted(actual)} expected={sorted(expected)}'
        for key in expected:
            issue = mismatch(actual[key], expected[key], path + '.' + key)
            if issue:
                return issue
    elif isinstance(expected, list):
        if len(actual) != len(expected):
            return f'{path}: length {len(actual)}, expected {len(expected)}'
        for i, (a, e) in enumerate(zip(actual, expected)):
            issue = mismatch(a, e, f'{path}[{i}]')
            if issue:
                return issue
    elif actual != expected:
        return f'{path}: {str(actual)[:180]!r}, expected {str(expected)[:180]!r}'
    return None


def make_cases(original, root):
    cases = []
    for name, updates in [('public', {}), ('zero-budget', {'budget_units': 0}),
                          ('changed-policy', {'budget_units': 17, 'shipping_saving_bp': 3333,
                                              'discount_threshold_ppm': 99999, 'discount_recovery_bp': 1234})]:
        folder = root / (name + ' input with spaces')
        shutil.copytree(original, folder)
        policy = json.loads((folder / 'policy.json').read_text())
        policy.update(updates)
        (folder / 'policy.json').write_text(json.dumps(policy))
        cases.append((name, folder))
    headers = list(csv_rows(original / 'Africa_orders.csv')[0])
    template = csv_rows(original / 'Africa_orders.csv')[0]
    for name in ['precision-events', 'ties', 'minimum-cost-tie', 'all-cancelled', 'empty']:
        folder = root / (name + ' input with spaces')
        shutil.copytree(original, folder)
        rows = []
        if name == 'precision-events':
            for sales, profit, shipping, discount in [('10.001', '2.333', '3.001', '0.200001'),
                                                      ('6.001', '0.001', '1.001', '0.1'),
                                                      ('0', '-1', '0', '0.6'),
                                                      ('1', '1', '0', '0.2'),
                                                      ('0.010', '0.001', '0.001', '0.200000')]:
                rows.append(dict(template, **{'Order ID': 'DUPLICATE', 'Sales': sales, 'Profit': profit,
                                              'Shipping Cost': shipping, 'Discount': discount,
                                              'Ship Mode': 'First Class', 'Order Priority': 'Low'}))
        elif name in ['ties', 'minimum-cost-tie']:
            rows = [dict(template, **{'Sales': '1', 'Profit': '0', 'Shipping Cost': '1',
                                     'Discount': '0.6', 'Ship Mode': 'First Class', 'Order Priority': 'Low'})
                    for _ in range(1 if name == 'minimum-cost-tie' else 3)]
        elif name == 'all-cancelled':
            rows = [dict(template)]
        for market in MARKETS:
            with (folder / (market + '_orders.csv')).open('w', newline='') as f:
                writer = csv.DictWriter(f, fieldnames=headers)
                writer.writeheader()
                if market == 'Africa':
                    writer.writerows(rows)
        events = []
        if name == 'precision-events':
            for i, (event_id, row_number, kind, amount, timestamp) in enumerate([
                ('a', 1, 'refund', 1237, '03'), ('b', 2, 'refund', 1, '04'),
                ('c', 2, 'cancel', 0, '02'), ('a', 1, 'cancel', 0, '01'),
                ('d', 1, 'shipping_credit', 1000, '04'), ('e', 1, 'refund', True, '05'),
                ('f', 1, 'cancel', False, '06'), ('g', 99, 'bad_type', -1, '01'),
                ('h', 2, 'bad_type', -1, '06'), ('i', 1, 'refund', 999999, '07'),
                ('j', 1, 'shipping_credit', 0.5, '08'), ('k', 3, 'cancel', 0, '01'),
                ('l', 5, 'cancel', 0, '01'), ('m', 4, 'refund', 1000, '09')]):
                events.append(dict(event_id=event_id, row_key=f'Africa_orders.csv:{row_number:06}',
                                   type=kind, amount_milli=amount, timestamp=f'2026-01-01T00:00:{timestamp}Z'))
        elif name == 'all-cancelled':
            events = [dict(event_id=event_id, row_key='Africa_orders.csv:000001', type=kind,
                           amount_milli=amount, timestamp='2026-01-01T00:00:01Z')
                      for event_id, kind, amount in [('z-refund', 'refund', 1), ('a-cancel', 'cancel', 0)]]
        (folder / 'adjustments.jsonl').write_text('\n\n'.join(json.dumps(e) for e in events) + '\n')
        policy = json.loads((folder / 'policy.json').read_text())
        if name in ['ties', 'minimum-cost-tie']:
            policy.update(budget_units=2, shipping_saving_bp=10000, discount_recovery_bp=10000,
                          shipping_cost_by_market={m: 2 if name == 'minimum-cost-tie' else 1 for m in MARKETS},
                          discount_cost_by_market={m: 1 for m in MARKETS})
        (folder / 'policy.json').write_text(json.dumps(policy))
        cases.append((name, folder))
    return cases


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--run-root', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    args.run_root.mkdir(parents=True, exist_ok=False)
    submission = args.run_root / 'isolated submission'
    submission.mkdir()
    source = args.project / 'pipeline.py'
    pipeline = submission / source.name
    shutil.copyfile(source, pipeline)
    tree = ast.parse(pipeline.read_text())
    imports = [node.module.split('.')[0] for node in ast.walk(tree) if isinstance(node, ast.ImportFrom) and node.module]
    imports += [alias.name.split('.')[0] for node in ast.walk(tree) if isinstance(node, ast.Import) for alias in node.names]
    third_party = sorted(set(imports) - sys.stdlib_module_names - {'__future__'})
    results = []
    for name, folder in make_cases(args.project / 'input', args.run_root):
        try:
            expected = oracle(folder)
            before = sha_files(folder)
            bytes_runs = []
            for repeat in range(2):
                output = args.run_root / f'{name} output {repeat} with spaces'
                process = subprocess.run([sys.executable, str(pipeline), '--input', str(folder), '--output', str(output)],
                                         cwd=args.run_root, capture_output=True, text=True, timeout=60)
                if process.returncode:
                    raise AssertionError(f'exit {process.returncode}: {process.stderr[-1500:]}')
                content = (output / 'results.json').read_bytes()
                bytes_runs.append(content)
                issue = mismatch(json.loads(content), expected)
                if issue:
                    raise AssertionError(issue)
                assert sha_files(folder) == before, 'Input file bytes changed'
            assert bytes_runs[0] == bytes_runs[1], 'Output bytes differ between identical runs'
            results.append(dict(case=name, passed=True, rows=len(expected['normalized_rows']),
                                events=len(expected['event_audit']), candidates=len(expected['action_candidates']),
                                optimization=expected['optimization']))
        except Exception as error:
            results.append(dict(case=name, passed=False, error=str(error)))
    report = dict(submission_sha256=hashlib.sha256(pipeline.read_bytes()).hexdigest(),
                  third_party_imports=third_party, isolated_single_file=True, cases=results,
                  passed=not third_party and all(r['passed'] for r in results))
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
