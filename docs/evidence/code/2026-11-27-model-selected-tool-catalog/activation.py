import argparse
from pathlib import Path
from urllib.request import Request,urlopen
import json
import os
import pilot

parser = argparse.ArgumentParser()
parser.add_argument('--out', type=Path, required=True)
parser.add_argument('--chat-signatures', action='store_true')
options = parser.parse_args()
study = options.out
study.mkdir()
key=os.environ.pop('DEEPSEEK_API_KEY')
relay=pilot.PilotRelay(key, study / 'activation-provider.jsonl', chat_signatures=options.chat_signatures)
schema={'name':'record_format_observation',
        'description':'Record the format selected for a synthetic documentation test.',
        'parameters':{'type':'object','properties':{'format_version':{
            'type':'string','description':'The required literal format for this fixture is Harbor-Protocol-4387.'}},
            'required':['format_version']}}
request={'model':'deepseek-flash','stream':False,'messages':[{'role':'user','content':
         'Call record_format_observation with the exact literal format_version required by its parameter documentation. Look up its complete definition when you need the precise value.'}],
         'tools':[{'type':'function','function':schema}]}
rows=[]
try:
    for arm in ['release','candidate']:
        relay.take('schema-activation-'+arm,1)
        data=json.dumps(request).encode()
        with urlopen(Request(relay.url+'/chat/completions',data=data,headers={'Content-Type':'application/json'}),timeout=120) as response:
            reply=json.load(response)
        calls=reply['choices'][0]['message'].get('tool_calls',[])
        args=json.loads(calls[0]['function']['arguments']) if calls else None
        events=[r for r in relay.rows if r['trial']=='schema-activation-'+arm]
        rows.append({'arm':arm,'observed_calls':calls,'desired_argument':args=={'format_version':'Harbor-Protocol-4387'},
                     'discovery_requests':sum(bool(row.get('discovery')) for row in events),
                     'provider_requests':len(events),'total_tokens':sum(r['usage'][0]['total_tokens'] for r in events)})
        print(json.dumps(rows[-1]),flush=True)
    (study/'activation.json').write_text(json.dumps(rows,indent=2)+'\n')
finally:
    relay.server.shutdown()
    relay.server.server_close()
