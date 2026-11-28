"""Run the unchanged ACP task and grader with model-selected deferred definitions.

The relay rewrites only Aster tool-bearing requests. ACP executes every real
call. The relay responds only to the synthetic definition lookup and tracks
its provider requests individually. DSH and auxiliary calls use the original
Chat Completions route.
"""
from contextlib import closing
from collections import Counter
from hashlib import sha256
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from importlib.util import module_from_spec, spec_from_file_location
import argparse
import copy
import http.client
import json
import os
from pathlib import Path
import threading
import time

DRIVER = Path(__file__).resolve().parent.parent / '2026-11-27-deepseek-harness-token-comparison/compare.py'
spec = spec_from_file_location('frozen_benchmark', DRIVER)
base = module_from_spec(spec)
spec.loader.exec_module(base)
MODEL = base.MODEL
LOOKUP = 'load_tool_definitions'


def fingerprint(messages):
    return sha256(json.dumps(messages, sort_keys=True, ensure_ascii=False).encode()).hexdigest()


def upstream(path, body, key):
    with closing(http.client.HTTPSConnection('api.deepseek.com', timeout=60)) as conn:
        conn.request('POST', path, json.dumps(body, ensure_ascii=False).encode(), {
            'Authorization':'Bearer '+key,
            'Content-Type':'application/json',
            'anthropic-version':'2023-06-01',
        })
        reply = conn.getresponse()
        return reply.status, json.loads(reply.read())


def as_physical_tool(tool):
    fn = tool['function']
    return {'name':fn['name'], 'description':fn.get('description', ''),
            'input_schema':fn.get('parameters', {'type':'object'})}


def signature(schema):
    if not isinstance(schema, dict):
        return '?'
    if 'const' in schema:
        return json.dumps(schema['const'],ensure_ascii=False,separators=(',',':'))
    if schema.get('enum') and all(not isinstance(v,(dict,list)) for v in schema['enum']):
        return '|'.join(str(v) for v in schema['enum'])
    if isinstance(schema.get('properties'),dict):
        required=set(schema.get('required',[]))
        return '{'+', '.join(name+('' if name in required else '?')+':'+signature(child)
                            for name,child in schema['properties'].items())+'}'
    if schema.get('type')=='array':
        return '['+signature(schema.get('items'))+']'
    kind=schema.get('type')
    if isinstance(kind,list):
        return '|'.join(str(t) for t in kind)
    return {'string':'str','integer':'int','number':'num','boolean':'bool','object':'obj'}.get(kind,'value') if isinstance(kind,str) else 'value'


def card(tool):
    title=tool['description'].split('\n',1)[0][:150]
    return tool['name']+signature(tool['input_schema'])+' — '+title


def annotated_signature(schema):
    if not isinstance(schema, dict):
        return signature(schema)
    branches = next((schema[key] for key in ('oneOf', 'anyOf', 'allOf') if key in schema), None)
    if isinstance(branches, list):
        operator = '&' if 'allOf' in schema else '|'
        shape = '(' + operator.join(annotated_signature(branch) for branch in branches) + ')'
    elif isinstance(schema.get('properties'), dict):
        required = set(schema.get('required', []))
        shape = '{' + ','.join(
            name + ('' if name in required else '?') + ':' + annotated_signature(child)
            for name, child in schema['properties'].items()
        ) + '}'
    elif schema.get('type') == 'array':
        shape = '[' + annotated_signature(schema.get('items')) + ']'
    else:
        shape = signature(schema)
    note = schema.get('description', '')
    if isinstance(note, str) and note and '\n' not in note and len(note.encode()) <= 128:
        shape += ' (' + note + ')'
    return shape


def schema_without_annotations(schema):
    """Keep every structural constraint and remove documentation at schema nodes."""
    if not isinstance(schema, dict):
        return schema
    output = {}
    for key, value in schema.items():
        if key in ('title', 'description', '$comment', 'examples'):
            continue
        if key in ('properties', 'patternProperties', 'dependentSchemas', '$defs', 'definitions', 'dependencies') and isinstance(value, dict):
            value = {name: schema_without_annotations(child) for name, child in value.items()}
        elif key in ('anyOf', 'oneOf', 'allOf', 'prefixItems', 'items') and isinstance(value, list):
            value = [schema_without_annotations(child) for child in value]
        elif key in ('items', 'additionalProperties', 'additionalItems', 'unevaluatedItems',
                     'unevaluatedProperties', 'contains', 'propertyNames', 'not', 'if',
                     'then', 'else', 'contentSchema'):
            value = schema_without_annotations(value)
        output[key] = value
    return output


def required_tool_parameters(schema, mode):
    if not isinstance(schema, dict):
        return {'type': 'object'}
    properties = schema.get('properties', {})
    required = schema.get('required', [])
    if (not isinstance(required, list) or not required or not isinstance(properties, dict)
            or not all(isinstance(name, str) and name in properties for name in required)):
        return {'type': 'object'}
    declarations = {}
    for name in required:
        field = properties[name]
        if mode == 'enum' and not (isinstance(field, dict) and ('enum' in field or 'const' in field)):
            continue
        if mode in ('root', 'enum') and isinstance(field, dict):
            field = {key: field[key] for key in ('type', 'enum', 'const') if key in field}
        else:
            field = schema_without_annotations(field)
        declarations[name] = field
    if not declarations:
        return {'type': 'object'}
    return {'type': 'object', 'required': list(declarations), 'properties': declarations}


def chat_tool_catalog(tools, preserve_schema, annotate=False, native_required=False):
    compact = []
    for tool in tools:
        schema = tool['input_schema']
        if preserve_schema:
            parameters = schema_without_annotations(schema)
        elif native_required:
            parameters = required_tool_parameters(schema, native_required)
        else:
            parameters = {'type': 'object'}
        purpose = tool['description'].split('\n', 1)[0][:150]
        if annotate:
            purpose += '\n' + annotated_signature(tool['input_schema'])
        elif not preserve_schema:
            purpose = card(tool)
        compact.append({'type': 'function', 'function': {
            'name': tool['name'], 'description': purpose, 'parameters': parameters,
        }})
    guidance = 'Open the exact definitions for one or more known tools. '
    if not preserve_schema:
        guidance += 'Signatures use ? for optional parameters. '
    guidance += 'Use this lookup alone if you need constraints, detailed formats, or examples.'
    compact.append({'type': 'function', 'function': {
        'name': LOOKUP,
        'description': guidance,
        'parameters': {'type': 'object', 'properties': {'names': {
            'type': 'array', 'items': {'type': 'string'},
        }}, 'required': ['names']},
    }})
    return compact


def json_bytes(value):
    return len(json.dumps(value, ensure_ascii=False, separators=(',', ':')).encode())


def text_blocks(content):
    if isinstance(content, str):
        return [{'type':'text','text':content}] if content else []
    if content is None:
        return []
    if all(isinstance(block, dict) and block.get('type') == 'text' for block in content):
        return content
    raise ValueError('pilot accepts text and native tool results only')


def from_chat(messages, anchors, offered):
    system = []
    translated = []
    loaded = set()
    if fingerprint([]) in anchors:
        raise ValueError('unexpected empty discovery prefix')
    for end, message in enumerate(messages, 1):
        role = message['role']
        if role == 'system':
            system.extend(text_blocks(message.get('content')))
        elif role == 'tool':
            block = {'type':'tool_result','tool_use_id':message['tool_call_id'],
                     'content':text_blocks(message['content'])}
            if translated and translated[-1]['role'] == 'user' and all(b['type']=='tool_result' for b in translated[-1]['content']):
                translated[-1]['content'].append(block)
            else:
                translated.append({'role':'user','content':[block]})
        elif role in ('user','assistant'):
            blocks = text_blocks(message.get('content'))
            for call in message.get('tool_calls', []):
                f = call['function']
                blocks.append({'type':'tool_use','id':call['id'],'name':f['name'],
                               'input':json.loads(f['arguments'])})
            if blocks:
                translated.append({'role':role,'content':blocks})
        else:
            raise ValueError('unsupported pilot message role')
        for record in anchors.get(fingerprint(messages[:end]), []):
            current = [name for name in record['names'] if offered.get(name)==record['definitions'][name]]
            translated.append({'role':'assistant','content':[record['call']]})
            translated.append({'role':'user','content':[{
                'type':'tool_result','tool_use_id':record['call']['id'],
                'content':[{'type':'tool_reference','tool_name':name} for name in current]
                          or [{'type':'text','text':'No requested definition is authorized in this request.'}]}]})
            loaded.update(current)
    return system, translated, loaded


def chat_usage(usage, native):
    if not native:
        return {k:usage[k] for k in base.USAGE}
    misses=usage['input_tokens']+usage.get('cache_creation_input_tokens',0)
    hits=usage.get('cache_read_input_tokens',0)
    completion=usage['output_tokens']
    return dict(prompt_tokens=misses+hits, completion_tokens=completion,
                total_tokens=misses+hits+completion,
                prompt_cache_hit_tokens=hits, prompt_cache_miss_tokens=misses)


class PilotRelay(base.Relay):
    def __init__(self, credential, logfile, eager_names=(), chat_signatures=False, cost_gate=False,
                 preserve_schema=False, annotate_signatures=False, native_required=False,
                 max_tokens=1024, max_requests=None, keep_full=None):
        self.active = None
        self.lock = threading.Lock()
        self.rows = []
        self.started = Counter()
        self.pending = Counter()
        upstream_started = Counter()
        self.credential = credential
        self.logfile = logfile
        self.eager_names = frozenset(eager_names)
        self.chat_signatures = chat_signatures
        self.cost_gate = cost_gate
        self.preserve_schema = preserve_schema
        self.annotate_signatures = annotate_signatures
        self.native_required = native_required
        anchors_by_trial = {}
        relay=self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self,*_):
                pass

            def do_GET(self):
                if self.path not in ('/models','/v1/models'):
                    self.send_error(404)
                    return
                self.finish_json(200,{'object':'list','data':[{'id':MODEL,'object':'model','owned_by':'deepseek'}]})

            def finish_json(self,status,body):
                self.send_response(status)
                self.send_header('Content-Type','application/json')
                self.send_header('Connection','close')
                self.end_headers()
                self.wfile.write(json.dumps(body,ensure_ascii=False).encode())

            def deliver(self,request,reply):
                if not request.get('stream'):
                    self.finish_json(200,reply)
                    return
                self.send_response(200)
                self.send_header('Content-Type','text/event-stream')
                self.send_header('Connection','close')
                self.end_headers()
                choice=reply['choices'][0]
                delta=choice['message']
                if delta.get('tool_calls'):
                    delta['tool_calls']=[{'index':i,**call} for i,call in enumerate(delta['tool_calls'])]
                chunk={'object':'chat.completion.chunk','id':reply['id'],'model':MODEL,
                       'choices':[{'index':0,'delta':delta,'finish_reason':None}]}
                self.wfile.write(b'data: '+json.dumps(chunk,ensure_ascii=False).encode()+b'\n\n')
                chunk['choices']=[{'index':0,'delta':{},'finish_reason':choice['finish_reason']}]
                chunk['usage']=reply['usage']
                self.wfile.write(b'data: '+json.dumps(chunk,ensure_ascii=False).encode()+b'\n\ndata: [DONE]\n\n')

            def do_POST(self):
                if self.path not in ('/chat/completions','/v1/chat/completions'):
                    self.send_error(404)
                    return
                size=int(self.headers.get('Content-Length',0))
                if not 0<size<=4*1024*1024:
                    self.send_error(413)
                    return
                request=json.loads(self.rfile.read(size))
                with relay.lock:
                    binding=relay.active
                if request.get('model')!=MODEL or binding is None:
                    self.send_error(409)
                    return
                with relay.lock:
                    relay.started[binding[0]] += 1
                    relay.pending[binding[0]] += 1
                try:
                    self.forward(request, binding)
                finally:
                    with relay.lock:
                        relay.pending[binding[0]] -= 1

            def forward(self, request, binding):
                label=binding[0]
                arm=label.rsplit('-',1)[-1]
                trees=request.get('tools') or []
                tools={x['function']['name']:as_physical_tool(x) for x in trees}
                eager = relay.eager_names | frozenset(
                    name for name, definition in tools.items()
                    if keep_full is not None and keep_full(definition)
                )
                ordinary=bool(tools) and request.get('tool_choice') in (None,'auto') and not request.get('response_format')
                native=ordinary and arm!='dsh' and not relay.chat_signatures
                deferred=ordinary and arm=='candidate' and LOOKUP not in tools
                compact = (chat_tool_catalog(tools.values(), relay.preserve_schema, relay.annotate_signatures,
                                             relay.native_required)
                           if deferred and relay.chat_signatures else None)
                if deferred and relay.cost_gate:
                    original_bytes=json_bytes(trees)
                    card_bytes=(json_bytes(compact) if relay.preserve_schema or relay.annotate_signatures
                                else sum(len(card(t).encode()) for t in tools.values()))
                    system_bytes=sum(len(str(m['content']).encode()) for m in request['messages'] if m['role']=='system')
                    deferred=original_bytes-card_bytes>system_bytes
                previous=anchors_by_trial.setdefault(label,{}) if deferred else {}
                start=time.monotonic()
                try:
                    for step in range(9):
                        if native:
                            system,history,loaded=from_chat(request['messages'],previous,tools)
                            shown=list(copy.deepcopy(tools).values())
                            dormant=[t for t in shown if t['name'] not in eager] if deferred and step<8 else []
                            for t in dormant:
                                t['defer_loading']=True
                            if dormant:
                                catalog='\n'.join(card(tool) for tool in dormant)
                                shown.append({'name':LOOKUP,'description':
                                    'Tool signatures below use ? for optional fields. Call tools with the listed arguments when sufficient. To see exact constraints or learn an unfamiliar tool, call this lookup alone with its precise name(s).\n'+catalog,
                                    'input_schema':{'type':'object','properties':{'names':{'type':'array','items':{'type':'string'}}},'required':['names']}})
                            body={'model':MODEL,'system':system,'messages':history,'tools':shown,
                                  'thinking':{'type':'disabled'},'temperature':0,'max_tokens':max_tokens}
                            path='/anthropic/v1/messages'
                        else:
                            body={k:v for k,v in request.items() if k not in ('reasoning_effort','max_completion_tokens','top_p','stream_options')}
                            body.update(thinking={'type':'disabled'},temperature=0,max_tokens=max_tokens,stream=False)
                            path='/chat/completions'
                            dormant=[]
                            loaded=set()
                            if relay.chat_signatures and deferred and step<8:
                                dormant=list(tools.values())
                                body['tools']=compact
                                history=[]
                                for end,message in enumerate(request['messages'],1):
                                    history.append(message)
                                    for record in previous.get(fingerprint(request['messages'][:end]),[]):
                                        call=record['call']
                                        current=[name for name in record['names'] if tools.get(name)==record['definitions'][name]]
                                        loaded.update(current)
                                        history.append({'role':'assistant','content':None,'tool_calls':[{'id':call['id'],'type':'function','function':{'name':call['name'],'arguments':json.dumps(call['input'],ensure_ascii=False)}}]})
                                        history.append({'role':'tool','tool_call_id':call['id'],'content':json.dumps([tools[name] for name in current],ensure_ascii=False)})
                                body['messages']=history
                        with relay.lock:
                            admitted = max_requests is None or upstream_started[label] < max_requests
                            if admitted:
                                upstream_started[label] += 1
                        if not admitted:
                            relay.record({'status':429,'error':'pilot_request_budget','usage':[],'models':[]},binding)
                            self.send_error(429)
                            return
                        status,reply=upstream(path,body,relay.credential)
                        raw_usage=reply.get('usage')
                        usage=chat_usage(raw_usage,native) if raw_usage else None
                        calls=( [b for b in reply.get('content',[]) if b.get('type')=='tool_use'] if native and status==200 else [] )
                        if not native and dormant and status==200:
                            calls=[{'type':'tool_use','id':c['id'],'name':c['function']['name'],
                                    'input':json.loads(c['function']['arguments'])}
                                   for c in reply['choices'][0]['message'].get('tool_calls',[])]
                        selectors=[c for c in calls if c['name']==LOOKUP] if dormant else []
                        actual=[c for c in calls if c['name']!=LOOKUP]
                        names=( [c['name'] for c in calls] if native else [c['function']['name'] for x in reply.get('choices',[]) for c in x['message'].get('tool_calls',[])] )
                        relay.record({'status':status,'models':[reply.get('model')] if status==200 else [],
                                      'usage':[usage] if usage else [],'tool_calls':names,
                                      'tool_definitions':len(trees),'api':path,'discovery':bool(selectors),
                                      'deferred_definitions':len(dormant),'activated_before':sorted(loaded),
                                      'direct_deferred_calls':[c['name'] for c in actual if dormant and c['name'] not in loaded|eager],
                                      'elapsed_ms':round((time.monotonic()-start)*1000),
                                      'error':None if status==200 else str(reply.get('error',{}).get('type','unknown'))}, binding)
                        if status!=200:
                            self.finish_json(status,reply)
                            return
                        if selectors:
                            if actual:
                                raise ValueError('mixed discovery and real calls in pilot')
                            entries=previous.setdefault(fingerprint(request['messages']),[])
                            for call in selectors:
                                wanted=call.get('input',{}).get('names',[])
                                selected=[n for n in dict.fromkeys(wanted) if n in tools and n not in eager] if isinstance(wanted,list) and all(isinstance(n,str) for n in wanted) else []
                                entries.append({'call':call,'names':selected,'definitions':{n:tools[n] for n in selected}})
                            continue
                        if not native:
                            self.deliver(request,reply)
                            return
                        text=''.join(b['text'] for b in reply['content'] if b['type']=='text')
                        msg={'role':'assistant','content':text or None}
                        if actual:
                            msg['tool_calls']=[{'id':c['id'],'type':'function','function':{
                                'name':c['name'],'arguments':json.dumps(c['input'],ensure_ascii=False)}} for c in actual]
                        finish='tool_calls' if actual else 'length' if reply.get('stop_reason')=='max_tokens' else 'stop'
                        self.deliver(request,{'id':reply['id'],'model':MODEL,'choices':[{
                            'index':0,'finish_reason':finish,'message':msg}],'usage':usage})
                        return
                    raise ValueError('discovery budget exhausted')
                except (ValueError,TypeError,KeyError,OSError,http.client.HTTPException) as exc:
                    relay.record({'status':502,'error':str(exc),'usage':[],'models':[]},binding)
                    self.send_error(502)

        self.server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        threading.Thread(target=self.server.serve_forever,daemon=True).start()
        self.url=f'http://127.0.0.1:{self.server.server_port}'


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--eager',nargs='*', default=[])
    parser.add_argument('--repeated',action='store_true')
    parser.add_argument('--chat-signatures',action='store_true')
    parser.add_argument('--cost-gate',action='store_true')
    representation = parser.add_mutually_exclusive_group()
    representation.add_argument('--preserve-schema',action='store_true')
    representation.add_argument('--annotate-signatures',action='store_true')
    parser.add_argument('--native-required', nargs='?', const='tree', choices=('tree', 'root', 'enum'))
    args,rest=parser.parse_known_args()
    if (args.preserve_schema or args.annotate_signatures) and not args.chat_signatures:
        parser.error('Chat catalog representations require --chat-signatures')
    if args.native_required and not args.annotate_signatures:
        parser.error('--native-required requires --annotate-signatures')
    if args.repeated:
        records=[{'slot':i,'value':'C-'+sha256(f'catalog:{i}'.encode()).hexdigest()[:16]} for i in range(96)]
        base.COMMAND='cat inventory.json'
        base.FILES={'inventory.json':json.dumps({'items':records},separators=(',',':')).encode()+b'\n'}
        base.TASK=(('Run the shell command `cat inventory.json` exactly. Quote the value for slot 17 verbatim.',(records[17]['value'],)),
                   ('Independently verify the current file by running the same shell command `cat inventory.json` again exactly. Quote the value for slot 77 verbatim.',(records[77]['value'],)))
    base.Relay=lambda credential, logfile: PilotRelay(credential,logfile,args.eager,args.chat_signatures,args.cost_gate,
                                                      args.preserve_schema,args.annotate_signatures,args.native_required)
    import sys
    sys.argv=[sys.argv[0],*rest]
    base.main()


if __name__=='__main__':
    main()
