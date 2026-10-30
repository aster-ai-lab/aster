"""Freeze the range-exact unmark replay without altering the previous completed proof."""
import ast
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
worker = (B / 'literal_scan_cpu_worker_v143.frozen.py').read_text().replace('v143', 'v145')
worker = worker.replace('original_has, original_mark = D.F._has_tag, D.F._mark',
    'original_has, original_mark = D.F._has_tag, D.F._mark\n    original_unmark = D.F.unmark_think_literals')
worker = worker.replace('cache = L.install(D.F)', 'cache = L.install(D.F, D.S)')
anchor = "            D.F._has_tag, D.F._mark = original_has, original_mark"
worker = worker.replace(anchor, anchor + '\n            D.F.unmark_think_literals = original_unmark\n            D.S.unmark_think_literals = original_unmark')
anchor = "            if D.F._has_tag('<think>') != original_has('<think>') or D.F._mark('<think>') != original_mark('<think>'):"
if worker.count(anchor) != 1:
    raise RuntimeError('unique frozen CPU replay default-argument anchor differs')
validation = '''            import serve.test_frontend as upstream_tests
            upstream_log = io.StringIO()
            upstream = unittest.TextTestRunner(stream=upstream_log, verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(upstream_tests))
            (ROOT / 'upstream-frontend-tests.log').write_text(upstream_log.getvalue())
            STATE['unchanged_upstream_frontend_methods_passed'] = upstream.testsRun
            if not upstream.wasSuccessful():
                raise RuntimeError('unchanged upstream frontend acceptance failed with exact scanner enabled')
'''
worker = worker.replace(anchor, validation + anchor)
manager = (B / 'manage_literal_scan_cpu_v143.py').read_text().replace('v143', 'v145').replace('literal-scan-v142', 'literal-scan-v144')
manager = manager.replace("'unit-tests.log','fixture-metadata-canonical.json'", "'unit-tests.log','upstream-frontend-tests.log','fixture-metadata-canonical.json'")
rows = []
for name, text in [('literal_scan_cpu_worker_v145.frozen.py', worker), ('manage_literal_scan_cpu_v145.py', manager)]:
    raw = text.encode()
    ast.parse(raw, filename=name)
    with (B / name).open('xb') as output:
        output.write(raw)
    rows.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
manifest = dict(schema='aster-literal-scan-v144-cpu-v145', frozen_sources=rows)
(B / 'xolotl-literal-scan-v145-prepared.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(manifest, indent=2))
