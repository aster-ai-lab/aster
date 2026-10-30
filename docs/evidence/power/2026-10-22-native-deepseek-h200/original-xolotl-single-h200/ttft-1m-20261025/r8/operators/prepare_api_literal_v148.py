"""Freeze a default-off exact literal scanner overlay on the qualified f062 adapter."""
import ast
import difflib
import hashlib
import json
from xolotl_trial_common import BASE

root = BASE / 'api-adapter-v148'
if root.exists():
    raise RuntimeError('literal API overlay is already frozen')
original = (BASE / 'api-adapter-v52/serve_ds41.py').read_bytes()
if hashlib.sha256(original).hexdigest() != 'f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba':
    raise RuntimeError('exact qualified parent API differs')
source = original.decode()
anchor = 'from serve import server as S  # noqa: E402\n'
if source.count(anchor) != 1:
    raise RuntimeError('unique exact frontend/server import anchor differs')
source = source.replace(anchor, anchor + '\nfrom aster_literal_scan import install as _install_literal_scan\n_aster_literal_scan = _install_literal_scan(F, S)\n')
ast.parse(source)
root.mkdir()
(root / 'serve_ds41.py').write_bytes(source.encode())
(root / 'serve_ds41.original.py').write_bytes(original)
for name in ['aster_token_cache.py', 'test_exact_token_cache.py', 'test_strict_native_json.py', 'test_combined_adapter.py', 'test_hybrid_native_json.py', 'actual-tool-output.json']:
    (root / name).write_bytes((BASE / 'api-adapter-v52' / name).read_bytes())
for name in ['aster_literal_scan.py', 'test_literal_scan.py']:
    (root / name).write_bytes((BASE / 'literal-scan-v144' / name).read_bytes())
(root / 'literal-scan-v148.patch').write_bytes(''.join(difflib.unified_diff(original.decode().splitlines(True), source.splitlines(True), fromfile='v52/serve_ds41.py', tofile='v148/serve_ds41.py')).encode())
record = dict(schema='aster-exact-literal-overlay-v148', upstream_sha256=hashlib.sha256(original).hexdigest(),
    patched_sha256=hashlib.sha256(source.encode()).hexdigest(), helper_sha256=hashlib.sha256((root / 'aster_literal_scan.py').read_bytes()).hexdigest(),
    helper_source='literal-scan-v144', closed_startup_flag='ASTER_XQ_LITERAL_SCAN', closed_values=['0','1'], default_enabled=False,
    token_protection_changed=False, model_math_changed=False, memo_plain_span_bypass_preserved=True,
    request_text_retained_by_scanner=False, gpu_qualified=False, production_changed=False)
(root / 'overlay-plan.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
