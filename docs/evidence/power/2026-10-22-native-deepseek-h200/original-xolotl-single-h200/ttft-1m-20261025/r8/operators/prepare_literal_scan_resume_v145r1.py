"""Keep the failed harness receipt; isolate generic upstream tests from DS41 global hooks."""
import ast
import hashlib
import json
from pathlib import Path

B = Path(__file__).resolve().parent
worker = (B / 'literal_scan_cpu_worker_v145.frozen.py').read_text()
start = worker.index('            import serve.test_frontend as upstream_tests\n')
end = worker.index("            if D.F._has_tag('<think>')", start)
worker = worker[:start] + worker[end:]
anchor = '    import serve_ds41 as D\n'
if worker.count(anchor) != 1:
    raise RuntimeError('unique exact CPU API import anchor differs')
validation = '''    import subprocess
    child = "import sys,unittest;from pathlib import Path;sys.path[:0]=" + repr([str(ROOT), str(source / 'python-deps-preflight'), str(source / 'source')])
    child += ";from serve import frontend as F,server as S;import aster_literal_scan as L;assert L.install(F,S) is not None;import serve.test_frontend as T;result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(T));print('UPSTREAM_METHODS='+str(result.testsRun));sys.exit(0 if result.wasSuccessful() else 1)"
    upstream = subprocess.run([sys.executable, '-c', child], capture_output=True, text=True, timeout=30, cwd=ROOT)
    (ROOT / 'upstream-frontend-tests.log').write_text(upstream.stdout + upstream.stderr)
    if upstream.returncode:
        raise RuntimeError('isolated unchanged upstream frontend acceptance failed with exact scanner enabled')
    counts = [int(line.split('=',1)[1]) for line in upstream.stdout.splitlines() if line.startswith('UPSTREAM_METHODS=')]
    if len(counts) != 1 or counts[0] != 25:
        raise RuntimeError('exact isolated upstream test count differs')
    STATE['unchanged_upstream_frontend_methods_passed'] = counts[0]
    STATE['generic_tests_isolated_from_ds41_global_hooks'] = True
'''
worker = worker.replace(anchor, validation + anchor).replace('v145', 'v145r1')
manager = (B / 'manage_literal_scan_cpu_v145.py').read_text().replace('v145', 'v145r1')
rows = []
for name, text in [('literal_scan_cpu_worker_v145r1.frozen.py', worker), ('manage_literal_scan_cpu_v145r1.py', manager)]:
    raw = text.encode()
    ast.parse(raw, filename=name)
    with (B / name).open('xb') as output:
        output.write(raw)
    rows.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
record = dict(schema='aster-literal-scan-v145r1-harness-isolation', files=rows,
    previous_failure_retained='xolotl-literal-scan-cpu-v145-latest.json', candidate_source_changed=False,
    tests_isolated_because='DS41 API import globally replaces generic Service init/EOS/template hooks; ByteTokenizer tests require the original generic module')
(B / 'xolotl-literal-scan-v145r1-prepared.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
