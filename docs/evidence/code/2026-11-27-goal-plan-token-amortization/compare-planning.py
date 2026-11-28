#!/usr/bin/env python3
"""Measure real provider usage for the SDK's explicit goal and plan paths."""
import argparse
from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import re
import subprocess

DRIVER = Path(__file__).resolve().parent.parent / '2026-11-27-deepseek-harness-token-comparison' / 'compare.py'
spec = spec_from_file_location('compare', DRIVER)
driver = module_from_spec(spec)
spec.loader.exec_module(driver)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--isolate-tools', action='store_true')
    args = parser.parse_args()
    args.out.mkdir()
    relay = driver.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    config = (args.out / 'fixture.acl').resolve()
    config.write_text(f'''default_model = "deepseek/deepseek-flash"
reasoning_effort = "none"
providers "deepseek" {{
  api_key = "local-test-key"
  base_url = "{relay.url}"
  models "deepseek-flash" {{}}
}}
''')
    results = []
    orders = [('separate', 'combined'), ('combined', 'separate'), ('separate', 'combined')]
    try:
        for case in ['reconciliation', 'localization']:
            for repetition, order in enumerate(orders, 1):
                for arm in order:
                    label = f'{case}-{repetition}-{arm}'
                    relay.take(label, 1)
                    home = (args.out / label).resolve()
                    home.mkdir()
                    env = {'HOME': str(home), 'PATH': os.environ['PATH'], 'LANG': 'en_US.UTF-8',
                           'ASTER_CONFIG_FILE': str(config), 'ASTER_GOAL_PLAN_CASE': case,
                           'ASTER_GOAL_PLAN_ARM': arm}
                    if args.isolate_tools:
                        env['ASTER_GOAL_PLAN_ISOLATE_TOOLS'] = '1'
                    run = subprocess.run(
                        [str(args.binary), 'explicit_goal_plan_token_comparison',
                         '--ignored', '--exact', '--nocapture', '--test-threads=1'],
                        env=env,
                        capture_output=True, text=True, encoding='utf-8', timeout=240,
                    )
                    (home / 'test.log').write_text(run.stdout + run.stderr)
                    rows = [row for row in relay.rows if row.get('trial') == label]
                    planning = [row for row in rows if not row.get('stream') and not row.get('tool_definitions')]
                    summary = re.search(r'PLAN_COMPARISON criteria=(\d+) steps=(\d+)', run.stdout)
                    usage_complete = all(row.get('status') == 200 and row.get('models') == [driver.MODEL]
                                         and len(row.get('usage', [])) == 1 for row in rows)
                    usage_complete &= bool(rows) and all(
                        u['prompt_tokens'] + u['completion_tokens'] == u['total_tokens']
                        and u['prompt_cache_hit_tokens'] + u['prompt_cache_miss_tokens'] == u['prompt_tokens']
                        for row in rows for u in row['usage']
                    )
                    result = {
                        'case': case, 'repetition': repetition, 'arm': arm,
                        'tools_isolated': args.isolate_tools,
                        'planning_pass': bool(summary),
                        'task_pass': run.returncode == 0 and bool(summary),
                        'criteria': int(summary[1]) if summary else None,
                        'steps': int(summary[2]) if summary else None,
                        'usage_complete': usage_complete,
                        'planning_requests': len(planning),
                        'primary_requests': sum(bool(row.get('stream')) for row in rows),
                        'planning_tokens': {k: sum(row['usage'][0][k] for row in planning) for k in driver.USAGE}
                        if usage_complete else None,
                    }
                    results.append(result)
                    print(json.dumps(result), flush=True)
                    (args.out / 'trials.json').write_text(json.dumps(results, indent=2) + '\n')
    finally:
        relay.server.shutdown()
        relay.server.server_close()
    assert all(t['task_pass'] and t['usage_complete'] and t['planning_requests'] == (
        2 if t['arm'] == 'separate' else 1
    ) for t in results), results

if __name__ == '__main__':
    main()
