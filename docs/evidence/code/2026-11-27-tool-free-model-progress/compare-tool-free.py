#!/usr/bin/env python3
"""Compare tool-free turns on two versions of the same SDK test binary."""
import argparse
from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import subprocess

FIXTURE = Path(__file__).resolve().parent.parent / '2026-11-27-deepseek-harness-token-comparison' / 'compare.py'
spec = spec_from_file_location('frozen_relay', FIXTURE)
driver = module_from_spec(spec)
spec.loader.exec_module(driver)

def main():
    parser = argparse.ArgumentParser()
    for flag in ('release', 'candidate', 'out'):
        parser.add_argument('--' + flag, required=True, type=lambda value: Path(value).resolve())
    args = parser.parse_args()
    args.out.mkdir()
    relay = driver.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    config = args.out / 'fixture.acl'
    config.write_text(f'''default_model = "deepseek/deepseek-flash"
reasoning_effort = "none"
providers "deepseek" {{
  api_key = "local-test-key"
  base_url = "{relay.url}"
  models "deepseek-flash" {{}}
}}
''')
    results = []
    order = [('release', 'candidate'), ('candidate', 'release'), ('release', 'candidate')]
    try:
        for case in ('reconciliation', 'localization'):
            for repeat, arms in enumerate(order, 1):
                for arm in arms:
                    label = f'{case}-{repeat}-{arm}'
                    home = args.out / label
                    home.mkdir()
                    relay.take(label, 1)
                    child = subprocess.run(
                        [str(getattr(args, arm)), 'explicit_goal_plan_token_comparison',
                         '--ignored', '--exact', '--nocapture', '--test-threads=1'],
                        env={'HOME': str(home), 'PATH': os.environ['PATH'], 'LANG': 'en_US.UTF-8',
                             'ASTER_CONFIG_FILE': str(config), 'ASTER_GOAL_PLAN_CASE': case,
                             'ASTER_GOAL_PLAN_ARM': 'combined', 'ASTER_GOAL_PLAN_ISOLATE_TOOLS': '1'},
                        capture_output=True, text=True, encoding='utf-8', timeout=240,
                    )
                    (home / 'test.log').write_text(child.stdout + child.stderr)
                    rows = [r for r in relay.rows if r.get('trial') == label]
                    primary = [r for r in rows if r.get('stream')]
                    complete = bool(primary) and all(
                        r.get('status') == 200 and r.get('models') == [driver.MODEL]
                        and len(r.get('usage', [])) == 1 and r.get('tool_definitions') == 0
                        and r['usage'][0]['prompt_tokens'] + r['usage'][0]['completion_tokens'] == r['usage'][0]['total_tokens']
                        and r['usage'][0]['prompt_cache_hit_tokens'] + r['usage'][0]['prompt_cache_miss_tokens'] == r['usage'][0]['prompt_tokens']
                        for r in rows
                    )
                    result = {'trial': label, 'arm': arm, 'case': case,
                              'task_pass': child.returncode == 0 and 'PLAN_COMPARISON' in child.stdout,
                              'planning_pass': 'PLAN_COMPARISON' in child.stdout,
                              'usage_complete': complete,
                              'primary_requests': len(primary), 'all_requests': len(rows),
                              'primary_tokens': {k: sum(r['usage'][0][k] for r in primary) for k in driver.USAGE}
                              if complete else None,
                              'all_tokens': {k: sum(r['usage'][0][k] for r in rows) for k in driver.USAGE}
                              if complete else None}
                    if child.returncode:
                        for line in child.stderr.splitlines():
                            if 'main run did not complete' in line:
                                result['host_outcome'] = line
                                break
                    results.append(result)
                    print(json.dumps(result), flush=True)
                    (args.out / 'trials.json').write_text(json.dumps(results, indent=2) + '\n')
    finally:
        relay.server.shutdown()
        relay.server.server_close()

if __name__ == '__main__':
    main()
