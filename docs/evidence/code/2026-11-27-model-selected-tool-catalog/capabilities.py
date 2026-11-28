from pathlib import Path
import argparse
import json
import os
import subprocess
import pilot

parser=argparse.ArgumentParser()
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--chat-signatures',action='store_true')
parser.add_argument('--cost-gate',action='store_true')
args=parser.parse_args()
args.out.mkdir()
relay=pilot.PilotRelay(os.environ.pop('DEEPSEEK_API_KEY'),args.out/'provider-usage.jsonl',chat_signatures=args.chat_signatures,cost_gate=args.cost_gate)
config=args.out/'fixture.acl'
config.write_text(f'''default_model = "deepseek/deepseek-flash"
reasoning_effort = "none"
providers "deepseek" {{
  api_key = "local-test-key"
  base_url = "{relay.url}"
  models "deepseek-flash" {{}}
}}
''')
checks=(
 'deepseek_flash_search_finds_the_workspace_token',
 'deepseek_flash_glob_finds_the_nested_fixture',
 'deepseek_flash_git_diff_returns_the_changed_token',
 'deepseek_flash_program_returns_the_script_marker',
 'deepseek_flash_batch_reads_two_fixture_files',
 'deepseek_flash_ask_user_returns_the_host_answer',
 'deepseek_flash_edit_replaces_the_fixture_token',
 'deepseek_flash_patch_applies_the_given_hunk',
 'deepseek_flash_mcp_tool_returns_the_server_token',
 'deepseek_flash_skill_tool_returns_the_skill_token',
)
results=[]
try:
    for nth,case in enumerate(checks):
        for arm in (('release','candidate') if nth%2==0 else ('candidate','release')):
            name=f'{case}-{arm}'
            relay.take(name,1)
            home=args.out/name
            home.mkdir()
            child=subprocess.run([str(args.binary.resolve()),case,'--ignored','--exact','--nocapture','--test-threads=1'],
                                 env={'PATH':os.environ['PATH'],'HOME':str(home),'LANG':'en_US.UTF-8',
                                      'ASTER_CONFIG_FILE':str(config.resolve()),'ASTER_TEST_MODEL':'deepseek/deepseek-flash'},
                                 capture_output=True, text=True,encoding='utf-8',timeout=480)
            (home/'test.log').write_text(child.stdout+child.stderr)
            rows=[r for r in relay.rows if r['trial']==name]
            usage_complete=bool(rows) and all(r['status']==200 and r['models']==[pilot.MODEL] and len(r['usage'])==1 for r in rows)
            result={'case':case,'arm':arm,'pass':child.returncode==0 and '1 passed' in child.stdout,
                    'requests':len(rows),'usage_complete':usage_complete,
                    'discovery_requests':sum(bool(r.get('discovery')) for r in rows),
                    'tool_calls':[name for row in rows for name in row.get('tool_calls',[])],
                    'tokens':sum(r['usage'][0]['total_tokens'] for r in rows) if usage_complete else None}
            results.append(result)
            print(json.dumps(result),flush=True)
            (args.out/'trials.json').write_text(json.dumps(results,indent=2)+'\n')
finally:
    relay.server.shutdown()
    relay.server.server_close()
