"""Make the first small prefill chunk a complete, immediately yieldable checkpoint."""
import difflib,hashlib,json
from pathlib import Path
BASE=Path('D:/code/aster/target/h200-native');previous=BASE/'ttft-v13';root=BASE/'ttft-v15';root.mkdir(exist_ok=True)
original=(previous/'ds41_generate.cpp').read_text();s=original
def replace(old,new):
    global s
    if s.count(old)!=1:raise RuntimeError('unique initial checkpoint anchor differs: '+old[:80])
    s=s.replace(old,new)
replace('''                    cut = (cut / SB) * SB;
                    if (!enc_reported)''','''                    cut = (cut / SB) * SB;
                    // A complete first checkpoint can yield to a request that
                    // arrived during this batch, without another replay batch.
                    if (prefill_qos && c0 == 0 && n > 8192) cut = 0;
                    if (!enc_reported)''')
replace('''                c0 += B;
            } else {''','''                if (prefill_qos && c0 == 0 && n > 8192 && !yield_here && feed_yield && c0 + B < n)
                    yield_here = feed_yield(c0, B);
                c0 += B;
            } else {''')
(root/'ds41_generate.cpp').write_text(s,newline='\n')
for name in ['aster_prefill_policy.hpp','build_worker.py']:(root/name).write_bytes((previous/name).read_bytes())
test=(previous/'policy_test.cpp').read_text();needle='    bool enabled = true;'
extra='''    // Every layer of the first checkpoint is computed before publishing it;
    // the checkpoint remains entirely inside the final replay window even at 1M.
    for (int total : {8193, 35700, 131073, 1048512}) {
        const int first = aster_prefill::initial_width(8192, 0, total);
        assert(first == 1024 && first < 2560 && first < total);
        assert(aster_prefill::yield_width(first, 0, 128, 2560) == first);
        ++cases;
    }
'''
if test.count(needle)!=1:raise RuntimeError('checkpoint test anchor differs')
test=test.replace(needle,extra+needle);(root/'policy_test.cpp').write_text(test,newline='\n')
(root/'prefill-v11.patch').write_text(''.join(difflib.unified_diff((BASE/'xolotl-ttft-v11-ds41_generate.original.cpp').read_text().splitlines(keepends=True),s.splitlines(keepends=True),fromfile='a/tools/v41/engine/ds41_generate.cpp',tofile='b/tools/v41/engine/ds41_generate.cpp')),newline='\n')
(root/'prefill-first-checkpoint-v15.patch').write_text(''.join(difflib.unified_diff(original.splitlines(keepends=True),s.splitlines(keepends=True),fromfile='v13/ds41_generate.cpp',tofile='v15/ds41_generate.cpp')),newline='\n')
print(json.dumps(dict(phase='frozen',source_sha256=hashlib.sha256(s.encode()).hexdigest(),first_checkpoint_full_layers=True,early_drop_or_route_approximation=False)))
