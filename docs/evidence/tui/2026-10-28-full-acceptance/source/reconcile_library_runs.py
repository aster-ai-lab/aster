"""Retain exact native library failures and their independent repetitions."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent


def failures(relative):
    raw = (ROOT / relative).read_bytes()
    text = raw.decode('utf-8-sig')
    names = re.findall(r'^test (.+?) \.\.\. FAILED\r?$', text, re.M)
    assert len(names) == len(set(names))
    return set(names), hashlib.sha256(raw).hexdigest()


def isolated(relative):
    rows = json.loads((ROOT / relative / 'results.json').read_text())
    assert len(rows) == len({row['name'] for row in rows})
    return {row['name']: dict(
        exit_code=row['exit_code'],
        file=relative + '/' + row['log'],
        sha256=hashlib.sha256((ROOT / relative / row['log']).read_bytes()).hexdigest(),
    ) for row in rows}


candidate, candidate_hash = failures('windows/pager-lib-completion-facts-final.log')
baseline, baseline_hash = failures('windows/pager-upstream-5629-baseline.log')
linux, linux_hash = failures('linux/pager-lib-completion-facts-final.log')
win_single = isolated('windows/pager-isolated-5862')
base_single = isolated('windows/pager-upstream-5629-isolated')
linux_single = isolated('linux/pager-isolated-5862')
modern = isolated('windows/pager-isolated-modern-5862')
assert set(win_single) == candidate and set(base_single) == baseline
assert set(linux_single) == linux and set(modern) == candidate
assert all(row['exit_code'] != 0 for group in (win_single, base_single, linux_single) for row in group.values())
assert len(candidate) == len(baseline) == 165 and len(linux) == 19
assert len(candidate & baseline) == 164
owned = 'app::effects::tests::permission_ack_failure_narrows_ui_without_writing_preference'
assert candidate - baseline == {owned}
assert 'test result: ok. 1 passed;' in (ROOT / 'windows/permission-ack-fixed.log').read_text()
assert 'test result: ok. 1 passed;' in (ROOT / 'linux/permission-ack-fixed.log').read_text()
result = dict(
    schema='aster.tui.library-failure-attribution.v1',
    whole_pager_qualified=False,
    candidate_source_pair='5862dded8faeab96d4be04ae9ae535e3c474973e',
    upstream_commit='5629b3009ba757cb74a4b4463168ed480eca5553',
    log_sha256=dict(windows=candidate_hash, upstream_windows=baseline_hash, linux=linux_hash),
    counts=dict(windows_failures=len(candidate), upstream_windows_failures=len(baseline),
                shared_windows_failures=len(candidate & baseline), linux_failures=len(linux),
                modern_terminal_passes=sum(row['exit_code'] == 0 for row in modern.values())),
    candidate_only_failures=sorted(candidate - baseline),
    upstream_only_failures=sorted(baseline - candidate),
    owned_regression_correction=['windows/permission-ack-fixed.log', 'linux/permission-ack-fixed.log'],
    cases=[dict(name=name, candidate_windows=win_single.get(name), upstream_windows=base_single.get(name),
                candidate_linux=linux_single.get(name), explicit_modern_terminal=modern.get(name))
           for name in sorted(candidate | baseline | linux)],
    limitation='The baseline was executed on native Windows. Linux attribution is not inferred from matching names. No whole-library or whole-feature success is claimed.',
)
(ROOT / 'LIBRARY-FAILURES.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps(result['counts']))
