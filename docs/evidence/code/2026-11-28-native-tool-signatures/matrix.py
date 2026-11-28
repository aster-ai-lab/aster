#!/usr/bin/env python3
"""Run the original ACP fixtures with native names and compact argument signatures."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

evidence = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(evidence / '2026-11-27-model-selected-tool-catalog'))
sys.path.insert(0, str(evidence / '2026-11-28-default-catalog-capability-audit'))
sys.path.insert(0, str(evidence / '2026-11-27-six-turn-dsh-comparison'))
import full_catalog
import measure_form
import revisions
from measure_first import PROJECTIONS, compact

base = full_catalog.base


def failed_tools(messages, out, trial):
    # The Chat Completions transcript omits ToolResult.is_error. Match the
    # active wire results to their exact admitted invocations in the ACP log.
    start = max((index for index, row in enumerate(messages) if row['role'] == 'user'), default=-1)
    observed = {row['tool_call_id'] for row in messages[start + 1:] if row['role'] == 'tool'}
    failed = set()
    for workspace in (out / trial / 'workspace', out / 'workspace'):
        for journal in (workspace / '.aster' / 'sessions').glob('*/*/*.jsonl'):
            prepared = {}
            for line in journal.read_text().splitlines():
                try:
                    fact = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if fact['type'] == 'tool.prepared':
                    call = fact['payload']['prepared']
                    prepared[call['identity']['tool_call_id']] = call['tool_name']
                if fact['type'] == 'tool.result':
                    receipt = fact['payload']['receipt']
                    call_id = receipt['identity']['tool_call_id']
                    if call_id in observed and call_id in prepared and receipt['outcome'] == 'failed':
                        failed.add(prepared[call_id])
    return failed


def instrument_relay(style, preserve_native, capture_first, native_on_failure):
    # Keep the original relay, endpoint, controls, and graders. Only the
    # candidate's tool representation changes before serialization.
    filename = Path(base.__file__)
    source = filename.read_text()
    needle = "                wire = json.dumps(request, separators=(',', ':'), ensure_ascii=False).encode()"
    replacement = (
        "                converted = 0\n"
        "                if style != 'native' and tools and binding[0].endswith('-candidate'):\n"
        "                    promoted = failed_tools(messages, logfile.parent, binding[0]) if native_on_failure else set()\n"
        "                    tools, converted = compact(tools, style, set(preserve_native) | promoted)\n"
        "                    request['tools'] = tools\n"
        "                    identity['native_after_failure'] = sorted(promoted)\n"
        "                identity['projected_signatures'] = converted\n"
        + needle + "\n"
        "                if capture_first and tools and binding[1] == 1:\n"
        "                    capture = logfile.parent / f'{binding[0]}-first-wire.json'\n"
        "                    if not capture.exists():\n"
        "                        capture.write_bytes(wire)"
    )
    assert source.count(needle) == 1
    namespace = {**vars(base), 'compact': compact, 'style': style,
                 'preserve_native': preserve_native, 'capture_first': capture_first,
                 'native_on_failure': native_on_failure, 'failed_tools': failed_tools}
    exec(compile(source.replace(needle, replacement), str(filename), 'exec'), namespace)
    base.Relay = namespace['Relay']


async def forms(args):
    args.out.mkdir()
    args.fixture = 'contract'
    relay = base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl', max_requests=12)
    trials = []
    try:
        for index, order in enumerate((('release', 'candidate'), ('candidate', 'release')), 1):
            for arm in order:
                row = await measure_form.run_trial(f'{index}-{arm}', args.acp, args, relay)
                trials.append({'arm': arm, **row})
                (args.out / 'trials.json').write_text(json.dumps(trials, indent=2, ensure_ascii=False) + '\n')
                print(row['label'], row['passed'], row['tokens'], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scenario', required=True, choices=(
        'short', 'form', 'long', *full_catalog.SCENARIOS, 'program_approved', 'verified_edit_exact_count',
    ))
    parser.add_argument('--projection', choices=('native', *PROJECTIONS), default='signature')
    parser.add_argument('--reverse', action='store_true')
    parser.add_argument('--native-function', action='append', default=[], help='keep the exact named declaration for isolation')
    parser.add_argument('--capture-first', action='store_true', help='retain the synthetic short-task requests for input metering')
    parser.add_argument('--native-on-failure', action='store_true', help='restore declarations using typed failed receipts')
    for name in ('acp', 'dsh', 'embedding', 'out'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=name != 'dsh')
    args = parser.parse_args()
    if args.capture_first and args.scenario != 'short':
        parser.error('--capture-first is reserved for the synthetic short task')
    instrument_relay(args.projection, args.native_function, args.capture_first, args.native_on_failure)
    if args.scenario == 'short':
        assert args.dsh and args.dsh.is_file()
        args.release = args.candidate = args.acp
        args.order = ['candidate,release,dsh', 'dsh,release,candidate']
        asyncio.run(base.evaluate(args))
    elif args.scenario == 'form':
        asyncio.run(forms(args))
    elif args.scenario == 'long':
        assert args.dsh and args.dsh.is_file()
        revisions.candidate_with_catalog = lambda _directory, binary: binary
        args.reference_acp = args.acp
        args.reference_deferred = False
        args.fixture = evidence / '2026-11-27-six-turn-dsh-comparison' / 'fixture.json'
        args.order = ['dsh', 'release', 'candidate'] if args.reverse else ['candidate', 'release', 'dsh']
        asyncio.run(revisions.compare(args))
    else:
        args.reference_acp = args.acp
        args.default_catalog = True
        args.native_catalog = args.annotate_signatures = args.ungated = False
        args.native_required = None
        args.disk_content_attestation = args.scenario == 'verified_edit_exact_count'
        asyncio.run(full_catalog.evaluate(args))


if __name__ == '__main__':
    main()
