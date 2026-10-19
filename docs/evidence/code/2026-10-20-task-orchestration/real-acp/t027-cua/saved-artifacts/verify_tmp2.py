import random, sys
sys.path.insert(0, '.')
import pipeline


def brute(cands, budget):
    groups = {}
    for c in cands:
        groups.setdefault(c['row_key'], []).append(c)
    keys = sorted(groups)
    best = None
    def rec(i, cost, gain, ids):
        nonlocal best
        if i == len(keys):
            cand = (gain, cost, tuple(sorted(ids)))
            if best is None or pipeline.selection_better(cand, best):
                best = cand
            return
        rec(i + 1, cost, gain, ids)
        for c in groups[keys[i]]:
            if cost + c['cost_units'] <= budget:
                rec(i + 1, cost + c['cost_units'], gain + c['gain_milli'], ids + [c['action_id']])
    rec(0, 0, 0, [])
    return best


fails = 0
trials = 0
for seed in (7, 11, 23, 99):
    random.seed(seed)
    for trial in range(250):
        trials += 1
        n = random.randint(1, 9)
        cands = []
        for i in range(n):
            rk = 'f.csv:%06d' % (i + 1)
            for t in ('shipping_switch', 'discount_review'):
                if random.random() < 0.8:
                    cands.append({'action_id': rk + '|' + t, 'row_key': rk, 'type': t,
                                  'cost_units': random.choice([1, 2, 2, 3, 4]),
                                  'gain_milli': random.choice([10, 10, 20, 30, 30, 5])})
        cands.sort(key=lambda c: c['action_id'])
        budget = random.choice([0, 1, 2, 3, 5, 8, 12, 20])
        exp = brute(cands, budget)
        got_map, got_gain = pipeline.optimise(cands, budget)
        got = (got_gain, sum(c['cost_units'] for c in got_map.values()), tuple(sorted(got_map)))
        if exp != got:
            fails += 1
            if fails < 4:
                print('MISMATCH', seed, trial, 'budget', budget, 'exp', exp, 'got', got)
print('trials:', trials, 'mismatches:', fails)
