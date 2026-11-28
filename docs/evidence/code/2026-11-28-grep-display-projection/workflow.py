#!/usr/bin/env python3
"""Run the same search-heavy inspection on both Aster builds and standalone DSH."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-deepseek-harness-token-comparison'))
import compare

FILES = {
    f'region-{region}-production-service-rollout-safety-review.log': ''.join(
        f'window: WND-{offset + n}\n'
        f'phase-release: release-{region}-{n:02} route=RTE-{2 * offset + n}\n'
        f'recovery: REC-{3 * offset + n}\n'
        f'phase-rollback: rollback-{region}-{n:02} policy=POL-{4 * offset + n}\n'
        f'retention: RET-{5 * offset + n}\n\n' for n in range(1, 15)
    ).encode()
    for region, offset in [('north', 100), ('south', 250), ('west', 420)]
}
north, south, west = FILES
TASK = (
    ('Use a repository text search for the phase-release records in this rollout workspace, with the '
     'preceding context line. Identify release-south-09 and release-west-11. For each, give the '
     'source location in path:line format and the exact window and route codes.',
     (f'{south}:50', f'{west}:62', 'WND-259', 'RTE-509', 'WND-431', 'RTE-851')),
    ('Run a repository text search for phase-rollback records with the following context line. '
     'Identify rollback-north-04 and rollback-west-12. Report each source location in path:line '
     'format, its policy and retention codes, and the south release window from the first answer.',
     (f'{north}:22', f'{west}:70', 'POL-404', 'RET-504', 'POL-1692', 'RET-2112', 'WND-259')),
)


def permit(params, workspace):
    call = params.get('toolCall') or {}
    if call.get('kind') not in ('search', 'read'):
        return False
    raw = call.get('rawInput')
    if not isinstance(raw, dict):
        return False
    path = raw.get('path') or raw.get('file_path') or raw.get('filePath') or '.'
    return isinstance(path, str) and (workspace / path).resolve() in {workspace, *(workspace / file for file in FILES)}


if __name__ == '__main__':
    compare.FILES = FILES
    compare.TASK = TASK
    compare.safe_permission = permit
    compare.main()
