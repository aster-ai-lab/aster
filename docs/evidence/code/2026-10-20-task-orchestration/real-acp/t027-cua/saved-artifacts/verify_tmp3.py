import random, sys, pprint
sys.path.insert(0, '.')
import pipeline

random.seed(7)
for trial in range(400):
    n = random.randint(1, 7)
    cands = []
    for i in range(n):
        rk = 'f.csv:%06d' % (i + 1)
        for t in ('shipping_switch', 'discount_review'):
            if random.random() < 0.85:
                cands.append({'action_id': rk + '|' + t, 'row_key': rk, 'type': t,
                              'cost_units': random.choice([1, 2, 2, 3]),
                              'gain_milli': random.choice([10, 10, 20, 30])})
    cands.sort(key=lambda c: c['action_id'])
    budget = random.choice([0, 1, 3, 5, 8, 12])
    if trial == 79:
        pprint.pprint(cands)
        print('budget', budget)
        groups = {}
        for c in cands:
            groups.setdefault(c['row_key'], []).append(c)
        print('groups', {k: [(c['type'], c['cost_units'], c['gain_milli']) for c in v] for k, v in groups.items()})
        got_map, got_gain = pipeline.optimise(cands, budget)
        print('got', got_gain, sorted(got_map))
        break
