#!/usr/bin/env python3
"""Run ACP trials with unchanged tools, varying only their provider-side presentation."""

import argparse
import asyncio
from dataclasses import dataclass
from hashlib import sha256
import json
import os
from pathlib import Path
import sys

from pilot import MODEL, PilotRelay, base
from native import candidate_with_catalog

PROGRAM_SOURCE = 'async function run(ctx, inputs) { return { marker: [38 * 7, "VY"].join("-") }; }'
VERIFY_COMMAND = "test -f service.cfg && grep -Fx 'workers=12' service.cfg"
INITIAL_SERVICE = b'ticket=REED-17\nworkers=3\n'
EXPECTED_SERVICE = b'ticket=REED-17\nworkers=12\n'

@dataclass(frozen=True)
class Scenario:
    files: dict
    turns: tuple
    required_tool: str

    def manifest(self):
        return {
            'prompts': [prompt for prompt, _ in self.turns],
            'expected_markers': [markers for _, markers in self.turns],
            'fixtures_sha256': {name: sha256(data).hexdigest() for name, data in self.files.items()},
            'required_tool': self.required_tool,
        }


SCENARIOS = {
    'directory_discovery': Scenario(
        {'calibration-west.txt': b'Current calibration marker: WEST-PLUM-7296\n',
         'handoff.txt': b'Current handoff marker: LANTERN-57B\n',
         'unrelated-draft.txt': b'Preserve this unrelated draft.\n'},
        (
            ('List the workspace with ls to discover the calibration filename. Read that file. '
             'Use only ls and read in this turn. Return the exact calibration marker.', ('WEST-PLUM-7296',)),
            ('Use read only to read handoff.txt. Return its exact marker and the calibration marker '
             'from the first turn.', ('LANTERN-57B', 'WEST-PLUM-7296')),
        ),
        'ls',
    ),
    'read_window': Scenario(
        {'addresses.txt': b''.join(f'{n:02d} RECORD-{n * 7919:07d}\n'.encode() for n in range(1, 82))},
        (
            ('Use read with pagination to read only lines 43 through 45 of addresses.txt. '
             'Give their three record identifiers exactly.',
             ('RECORD-0340517', 'RECORD-0348436', 'RECORD-0356355')),
            ('Read only lines 70 through 71. Quote both current record identifiers and the one from line 44 above.',
             ('RECORD-0554330', 'RECORD-0562249', 'RECORD-0348436')),
        ),
        'read',
    ),
    'read_batch_repeat': Scenario(
        {
            'catalog notes.txt': b''.join(
                f'{n:03d} inventory item with lot {n * 2147:06d} and quality class {n % 7}\n'.encode()
                for n in range(1, 73)
            ) + b'routing_code=NORTH-TALLY-5984\n',
            'active.txt': b'handoff=PLAZA-840K\n',
            'backup.txt': b'backup=FERRY-237Q\n',
        },
        (
            ('Use one native read call with its files array to read catalog notes.txt and active.txt. '
             'Report the exact routing code and handoff.', ('NORTH-TALLY-5984', 'PLAZA-840K')),
            ('Use one fresh native read call with its files array to read backup.txt and catalog notes.txt. '
             'Report the exact backup and routing code.', ('FERRY-237Q', 'NORTH-TALLY-5984')),
        ),
        'read',
    ),
    'batch': Scenario(
        {'provider.conf': b'provider = copper-47VX\n', 'mirror.conf': b'mirror = amber-52LH\n',
         'policy.conf': b'approval = cedar-19QN\n'},
        (
            ('Read provider.conf and mirror.conf together using one native batch of two read calls. '
             'Report the exact provider and mirror values.', ('copper-47VX', 'amber-52LH')),
            ('Read policy.conf, then report its approval value together with the prior mirror value.',
             ('cedar-19QN', 'amber-52LH')),
        ),
        'batch',
    ),
    'batch_mixed': Scenario(
        {'ledger.ini': b'account=VELVET-49SX\n', 'export.log': b'process=VX-62 state=delivered\n',
         'checkpoint.txt': b'archive=MARBLE-24WM\n'},
        (
            ('In one native batch call, read ledger.ini and do a grep search for state=delivered '
             'in this workspace. Give the exact account and matching process values.',
             ('VELVET-49SX', 'VX-62')),
            ('Read checkpoint.txt and give the archive value alongside the previous process value.',
             ('MARBLE-24WM', 'VX-62')),
        ),
        'batch',
    ),
    'program': Scenario(
        {'outcome.log': b'postcondition=DUSK-Q4TP\n'},
        (
            ('Use the program tool in JavaScript script mode to evaluate this exact source. '
             'Report its computed marker.\n' + PROGRAM_SOURCE,
             ('266-VY',)),
            ('Read outcome.log and quote the postcondition together with the previous computed marker.',
             ('DUSK-Q4TP', '266-VY')),
        ),
        'program',
    ),
    'search': Scenario(
        {'importer.log': b'pending import batch=QA44 target=legacy\n',
         'exporter.log': b'finished export batch=QA44 target=mercury-R57K\n',
         'quality.log': b'checkpoint=4 verdict=indigo-W72Z\n'},
        (
            ('Search this workspace for the finished export of batch QA44. '
             'Report the matching filename and exact target, excluding unfinished imports.',
             ('exporter.log', 'mercury-R57K')),
            ('Read the quality checkpoint and quote its verdict together with the previously found target.',
             ('indigo-W72Z', 'mercury-R57K')),
        ),
        'search',
    ),
    'mcp_schema': Scenario(
        {'digest.txt': b'fixture=NODE-CLAY-29\n'},
        (
            ('Call mcp__fixture__observe_format with the exact format_version required by its '
             'parameter documentation. Open its full definition if needed. Report the returned marker.',
             ('SCHEMA-ACK-TEAL-2087',)),
            ('Read digest.txt and report its fixture value and the earlier exact tool marker.',
             ('NODE-CLAY-29', 'SCHEMA-ACK-TEAL-2087')),
        ),
        'mcp__fixture__observe_format',
    ),
    'verified_edit': Scenario(
        {'service.cfg': INITIAL_SERVICE},
        (
            ('Use edit to change workers from 3 to 12 in service.cfg, preserving all other bytes. '
             'Then run this exact bash verification command and report its output: `'
             + VERIFY_COMMAND + '`.', ('workers=12',)),
            ('Read service.cfg again and report its exact ticket and worker settings.',
             ('REED-17', 'workers=12')),
        ),
        'edit',
    ),
}

def program_permission(params, workspace):
    call = params.get('toolCall', {})
    raw = call.get('rawInput')
    allowed = [
        {'type': 'script', 'source': PROGRAM_SOURCE},
        {'type': 'script', 'language': 'javascript', 'source': PROGRAM_SOURCE},
    ]
    return (call.get('kind') == 'other' and raw in allowed) or original_permission(params, workspace)


original_permission = base.safe_permission
OriginalAcp = base.Acp


def observed_client(scenario, receipts, disk_content_attestation):
    class ObservedAcp(OriginalAcp):
        async def call(self, method, params, timeout=240):
            if method == 'session/new' and scenario == 'mcp_schema':
                params = {**params, 'mcpServers': [{
                    'name': 'fixture', 'command': sys.executable,
                    'args': [str(Path(__file__).resolve().with_name('mcp_fixture.py'))], 'env': [],
                }]}
            if method == 'session/new' and disk_content_attestation:
                params = {**params, '_meta': {'asterHost': {'completionAttestation': 'disk_content'}}}
            reply = await super().call(method, params, timeout)
            if method == 'session/prompt':
                receipts.append(reply[0].get('result', {}).get('_meta', {}).get('usage'))
            return reply

    return ObservedAcp


def mcp_permission(params, workspace):
    call = params.get('toolCall', {})
    return call.get('rawInput') == {'format_version': 'Harbor-Protocol-4387'} or original_permission(params, workspace)


def edit_permission(params, workspace, exact_count_allowed=False):
    call = params.get('toolCall', {})
    raw = call.get('rawInput')
    edit = {'file_path': 'service.cfg', 'old_string': 'workers=3', 'new_string': 'workers=12'}
    allowed = [edit, {**edit, 'replace_all': False}]
    if exact_count_allowed:
        allowed.extend({**shape, 'expected_replacements': 1} for shape in list(allowed))
    return (call.get('kind') == 'edit' and raw in allowed) or original_permission(params, workspace)


async def evaluate(args):
    scenario_name = {'program_approved': 'program', 'verified_edit_exact_count': 'verified_edit'}.get(args.scenario, args.scenario)
    scenario = SCENARIOS[scenario_name]
    args.out.mkdir()
    base.FILES = scenario.files
    base.TASK = scenario.turns
    base.COMMAND = object()
    if args.scenario == 'program_approved':
        base.safe_permission = program_permission
    elif args.scenario == 'mcp_schema':
        base.safe_permission = mcp_permission
    elif scenario_name == 'verified_edit':
        base.COMMAND = VERIFY_COMMAND
        base.safe_permission = lambda params, workspace: edit_permission(
            params, workspace, args.scenario == 'verified_edit_exact_count'
        )
    acp_usage = []
    base.Acp = observed_client(args.scenario, acp_usage, args.disk_content_attestation)
    credential = os.environ.pop('DEEPSEEK_API_KEY')
    usage_path = args.out / 'provider-usage.jsonl'
    release = args.reference_acp or args.acp
    candidate = args.acp
    if args.native_catalog or args.default_catalog:
        relay = base.Relay(credential, usage_path)
    else:
        relay = PilotRelay(credential, usage_path, chat_signatures=True, cost_gate=not args.ungated,
                           annotate_signatures=args.annotate_signatures, native_required=args.native_required)
    if args.native_catalog:
        candidate = candidate_with_catalog(args.out / 'launcher', args.acp)
        if args.reference_acp:
            release = candidate_with_catalog(args.out / 'reference', args.reference_acp)
    orders = [('release', 'candidate'), ('candidate', 'release')]
    manifest = {
        **scenario.manifest(),
        'scenario': args.scenario,
        'model': MODEL,
        'controls': {'thinking': 'disabled', 'temperature': 0, 'max_tokens': 1024},
        'approved_program': args.scenario == 'program_approved',
        'annotated_signatures': args.annotate_signatures,
        'native_required': args.native_required,
        'native_catalog': args.native_catalog,
        'default_catalog': args.default_catalog,
        'disk_content_attestation': args.disk_content_attestation,
        'reference_catalog': 'deferred' if args.native_catalog and args.reference_acp else
                             ('default' if args.default_catalog else 'full'),
        'cost_gate': not args.ungated,
        'expected_mutation_sha256': sha256(EXPECTED_SERVICE).hexdigest() if scenario_name == 'verified_edit' else None,
        'exact_count_allowed': args.scenario == 'verified_edit_exact_count',
        'orders': orders,
    }
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    rows = []
    try:
        for i, order in enumerate(orders, 1):
            for arm in order:
                label = f'{i}-{arm}'
                acp = candidate if arm == 'candidate' else release
                row = await base.run_trial(label, arm, acp, args.out, relay, args.embedding)
                events = [event for event in relay.rows if event['trial'] == label]
                main = [event for event in events if event.get('tool_definitions')]
                row['tool_catalog_sizes'] = [event['tool_definitions'] for event in main]
                row['compact_requests'] = sum(bool(event.get('deferred_definitions') or event.get('native_deferred_catalog')) for event in main)
                row['definition_queries'] = sum('load_tool_definitions' in event.get('tool_calls', []) for event in events)
                row['acp_prompt_usage'] = list(acp_usage)
                acp_usage.clear()
                receipts = []
                discovery_turns = []
                read_ranges = []
                verified_mutations = []
                for log in (args.out / label / 'workspace' / '.aster' / 'sessions').glob('*/*/*.jsonl'):
                    prepared = {}
                    for line in log.read_text().splitlines():
                        fact = json.loads(line)
                        if fact['type'] == 'tool.prepared':
                            tool = fact['payload']['prepared']
                            prepared[json.dumps(tool['identity'], sort_keys=True)] = tool['tool_name']
                        elif fact['type'] == 'tool.result':
                            receipt = fact['payload']['receipt']
                            name = prepared[json.dumps(receipt['identity'], sort_keys=True)]
                            receipts.append((name, receipt['outcome']))
                            if args.scenario == 'directory_discovery':
                                discovery_turns[-1].append((name, receipt['outcome']))
                        elif fact['type'] == 'user.message' and args.scenario == 'directory_discovery':
                            discovery_turns.append([])
                        elif fact['type'] == 'model.turn' and args.scenario == 'read_window':
                            body = fact['payload']
                            calls = [body.get('call', {}), *body.get('also', [])]
                            for call in calls:
                                if call.get('name') == 'read':
                                    params = call['args']
                                    read_ranges.append([params.get('file_path'), params.get('offset'), params.get('limit')])
                        elif fact['type'] == 'turn.complete' and fact['payload']['terminal'] == 'verified':
                            verified_mutations.append(fact['payload']['effect_digest'])
                row['receipt_outcomes'] = receipts
                if args.scenario == 'read_window':
                    row['read_ranges'] = read_ranges
                    row['read_window_exact'] = read_ranges == [['addresses.txt', 42, 3], ['addresses.txt', 69, 2]]
                row['required_tool_succeeded'] = (scenario.required_tool, 'succeeded') in receipts
                passed = row['task_pass']
                if args.scenario == 'directory_discovery':
                    row['discovery_turn_receipts'] = discovery_turns
                    row['allowed_tool_sequence'] = discovery_turns == [
                        [('ls', 'succeeded'), ('read', 'succeeded')], [('read', 'succeeded')]
                    ]
                    passed = passed and row['allowed_tool_sequence']
                if scenario_name == 'verified_edit':
                    edited_file = args.out / label / 'workspace' / 'service.cfg'
                    row['verified_mutations'] = verified_mutations
                    row['edit_bytes_match'] = edited_file.is_file() and edited_file.read_bytes() == EXPECTED_SERVICE
                    passed = (row['edit_bytes_match'] and len(verified_mutations) == 1
                              and ('bash', 'succeeded') in receipts and len(row['turns']) == 2
                              and all(turn['stop'] == 'end_turn' and not turn['missing_markers'] for turn in row['turns']))
                row['qualified_pass'] = (passed and row['required_tool_succeeded']
                                         and bool(main) and min(row['tool_catalog_sizes']) >= 20
                                         and (arm != 'candidate' or row['compact_requests'] > 0))
                rows.append(row)
                (args.out / 'trials.json').write_text(json.dumps(rows, indent=2) + '\n')
                print(json.dumps(row), flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--scenario', choices=(*SCENARIOS, 'program_approved', 'verified_edit_exact_count'), required=True)
    parser.add_argument('--annotate-signatures', action='store_true')
    parser.add_argument('--native-required', nargs='?', const='tree', choices=('tree', 'root', 'enum'))
    catalog = parser.add_mutually_exclusive_group()
    catalog.add_argument('--native-catalog', action='store_true', help='use Core’s native deferred catalog')
    catalog.add_argument('--default-catalog', action='store_true', help='use the ACP’s default catalog without a relay projection')
    parser.add_argument('--disk-content-attestation', action='store_true')
    parser.add_argument('--ungated', action='store_true')
    parser.add_argument('--acp', type=lambda text: Path(text).resolve(), required=True)
    parser.add_argument('--reference-acp', type=lambda text: Path(text).resolve())
    parser.add_argument('--embedding', type=lambda text: Path(text).resolve())
    parser.add_argument('--out', type=lambda text: Path(text).resolve(), required=True)
    args = parser.parse_args()
    if args.native_required and not args.annotate_signatures:
        parser.error('--native-required requires --annotate-signatures')
    if args.reference_acp and not (args.native_catalog or args.default_catalog):
        parser.error('--reference-acp requires --native-catalog or --default-catalog')
    asyncio.run(evaluate(args))


if __name__ == '__main__':
    main()
