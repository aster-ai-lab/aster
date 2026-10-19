import sys
sys.path.insert(0, '.')
import pipeline


cands = [
 {'action_id': 'f.csv:000001|discount_review','row_key':'f.csv:000001','type':'discount_review','cost_units':2,'gain_milli':10},
 {'action_id': 'f.csv:000002|discount_review','row_key':'f.csv:000002','type':'discount_review','cost_units':1,'gain_milli':20},
 {'action_id': 'f.csv:000002|shipping_switch','row_key':'f.csv:000002','type':'shipping_switch','cost_units':2,'gain_milli':30},
 {'action_id': 'f.csv:000003|shipping_switch','row_key':'f.csv:000003','type':'shipping_switch','cost_units':2,'gain_milli':20},
 {'action_id': 'f.csv:000004|discount_review','row_key':'f.csv:000004','type':'discount_review','cost_units':3,'gain_milli':30},
 {'action_id': 'f.csv:000004|shipping_switch','row_key':'f.csv:000004','type':'shipping_switch','cost_units':3,'gain_milli':20},
 {'action_id': 'f.csv:000005|discount_review','row_key':'f.csv:000005','type':'discount_review','cost_units':3,'gain_milli':30},
 {'action_id': 'f.csv:000005|shipping_switch','row_key':'f.csv:000005','type':'shipping_switch','cost_units':2,'gain_milli':20},
 {'action_id': 'f.csv:000006|discount_review','row_key':'f.csv:000006','type':'discount_review','cost_units':3,'gain_milli':10},
 {'action_id': 'f.csv:000006|shipping_switch','row_key':'f.csv:000006','type':'shipping_switch','cost_units':2,'gain_milli':10},
 {'action_id': 'f.csv:000007|discount_review','row_key':'f.csv:000007','type':'discount_review','cost_units':2,'gain_milli':20},
 {'action_id': 'f.csv:000007|shipping_switch','row_key':'f.csv:000007','type':'shipping_switch','cost_units':2,'gain_milli':10},
]
budget = 5

grouped = {}
for c in cands:
    grouped.setdefault(c['row_key'], []).append(c)
limit = min(budget, sum(max(i['cost_units'] for i in g) for g in grouped.values()))
states = {0: (0, ())}
for rk in sorted(grouped):
    group = sorted(grouped[rk], key=lambda i: i['action_id'])
    updated = dict(states)
    for cand in group:
        cost, gain = cand['cost_units'], cand['gain_milli']
        if cost > limit: continue
        for spent, (cg, ids) in list(states.items()):
            nc = spent + cost
            if nc > limit: continue
            ng = cg + gain
            nids = (cand['action_id'],) + ids
            ex = updated.get(nc)
            if ex is None or pipeline.selection_better((ng, nc, nids), (ex[0], nc, ex[1])):
                updated[nc] = (ng, nids)
    states = updated
    print('after', rk)
    for cst in sorted(states):
        print('  ', cst, states[cst])
