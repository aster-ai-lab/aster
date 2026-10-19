import json, itertools, random, sys
sys.path.insert(0, '.')

d = json.load(open('output/results.json'))
rows = {r['row_key']: r for r in d['normalized_rows']}

# --- 1. schema / field exactness
norm_keys = {"row_key","source_file","source_row","order_id","product_id","market","category",
             "sub_category","segment","ship_mode","priority","order_date","ship_date",
             "delivery_days","quantity","sales_milli","profit_milli","shipping_milli","discount_ppm"}
bad = [r['row_key'] for r in d['normalized_rows'] if set(r) != norm_keys]
print('normalized field mismatch:', len(bad))
eff_keys = norm_keys | {"active"}
bad = [r['row_key'] for r in d['effective_rows'] if set(r) != eff_keys]
print('effective field mismatch:', len(bad))

# --- 2. independent event replay using raw jsonl
raw = [l for l in open('input/adjustments.jsonl', encoding='utf-8') if l.strip()]
events = [json.loads(l) for l in raw]
seen = {}
order = []
for i, ev in enumerate(events, 1):
    if ev['event_id'] in seen:
        order.append((i, ev, 'duplicate_event'))
    else:
        seen[ev['event_id']] = i
        order.append((i, ev, None))
state = {r['row_key']: [True, r['sales_milli'], r['profit_milli'], r['shipping_milli']]
         for r in d['normalized_rows']}
todo = sorted([(e['timestamp'], e['event_id'], i, e) for i, e, dup in order if dup is None])
for ts, eid, i, ev in todo:
    rk = ev['row_key']
    amt = ev['amount_milli']
    if rk not in state:
        st = 'unknown_row'
    elif ev['type'] not in ('cancel','refund','shipping_credit'):
        st = 'unknown_type'
    elif not state[rk][0]:
        st = 'order_cancelled'
    else:
        cur = state[rk]
        ok_int = isinstance(amt, int) and not isinstance(amt, bool)
        if ev['type'] == 'cancel':
            if ok_int and amt == 0:
                state[rk] = [False, 0, 0, 0]; st = 'applied'
            else:
                st = 'invalid_amount'
        elif ev['type'] == 'refund':
            if ok_int and amt > 0 and amt <= cur[1]:
                cur[1] -= amt; cur[2] -= amt; st = 'applied'
            else:
                st = 'invalid_amount'
        else:
            if ok_int and amt > 0 and amt <= cur[3]:
                cur[3] -= amt; st = 'applied'
            else:
                st = 'invalid_amount'
    for j, (ii, ee, dup) in enumerate(order):
        if ii == i:
            order[j] = (ii, ee, st)
audit_ref = [{'input_line': i, 'event_id': e['event_id'], 'row_key': e['row_key'],
              'status': st} for i, e, st in order]
print('audit identical:', audit_ref == d['event_audit'])

eff_ref = []
for r in d['normalized_rows']:
    s = state[r['row_key']]
    e = dict(r)
    e['sales_milli'], e['profit_milli'], e['shipping_milli'] = s[1], s[2], s[3]
    e['active'] = s[0]
    eff_ref.append(e)
print('effective identical:', eff_ref == d['effective_rows'])

# --- 3. aggregate consistency: market sums == summary
agg = {k: {(tuple(v[f] for f in (['market'] if k=='market' else []))): v for v in d['aggregates'][k]}
       for k in ['market']}
s = d['summary']
for field in ['row_count','quantity','sales_milli','profit_milli','shipping_milli',
              'contribution_milli','high_discount_count','negative_contribution_count']:
    total = sum(v[field] for v in d['aggregates']['market'])
    assert total == s[field], (field, total, s[field])
print('market aggregate sums == summary: OK')

# market_month group count vs actual combos
combos = {(r['market'], r['order_date'][:7]) for r in d['normalized_rows']}
print('market_month groups:', len(d['aggregates']['market_month']), 'expected', len(combos))
print('all aggregate lists sorted:',
      all([ ( [ (tuple(v[f] for f in fields)) for v in d['aggregates'][name] ] ==
             sorted([ (tuple(v[f] for f in fields)) for v in d['aggregates'][name] ]) )
            for name, fields in [('market',['market']),('market_category',['market','category']),
                                 ('segment',['segment']),('ship_mode',['ship_mode']),
                                 ('market_month',['market','month'])] ]))

# --- 4. candidate constraints
cands = d['action_candidates']
print('candidates sorted:', [c['action_id'] for c in cands] == sorted(c['action_id'] for c in cands))
print('unique action ids:', len({c['action_id'] for c in cands}) == len(cands))
by_row = {}
for c in cands:
    by_row.setdefault(c['row_key'], []).append(c)
sel = d['selected_actions']
rowsel = [c['row_key'] for c in sel]
print('selection one per row:', len(rowsel) == len(set(rowsel)))
print('selection budget ok:', sum(c['cost_units'] for c in sel) <= d['optimization']['budget_units'])
print('selection subset of candidates:',
      all(c in cands for c in sel))
print('gain matches:', sum(c['gain_milli'] for c in sel) == d['optimization']['gain_milli'])

# --- 5. independent max-gain DP (simple, ignores lexicographic tie-break)
B = d['optimization']['budget_units']
groups = {}
for c in cands:
    groups.setdefault(c['row_key'], []).append(c)
best = {0: 0}
for rk in sorted(groups):
    nb = dict(best)
    for c in groups[rk]:
        for cost, g in best.items():
            nc = cost + c['cost_units']
            if nc <= B:
                nb[nc] = max(nb.get(nc, -1), g + c['gain_milli'])
    best = nb
maxgain = max(best.values())
mincost = min(c for c, g in best.items() if g == maxgain)
print('independent DP max gain:', maxgain, 'min cost:', mincost,
      '| pipeline:', d['optimization']['gain_milli'], d['optimization']['spent_units'])
