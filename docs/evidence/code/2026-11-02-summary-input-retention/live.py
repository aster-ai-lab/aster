from pathlib import Path
import datetime, hashlib, json, os, sys, time

root = Path.cwd()
out = root / 'docs/evidence/code/2026-11-02-summary-input-retention'
label = sys.argv[1]
sys.path.insert(0, str(root / 'crates/code/acp/tests/long_horizon'))
from timing_runner import Recorder, summarize, reconcile, compare

frozen = json.loads((out / (label + '-inputs.json')).read_text())
registration = Path(sys.argv[3]) if len(sys.argv) > 3 else out / 'qualification-preregistration.json'
registered = json.loads(registration.read_text())
attempts = out / (label + '-live')
attempts.mkdir()
inputs = root / 'docs/evidence/code/2026-11-01-agent-loop-general-optimization/independent-task'
with Recorder(attempts / 'admission-phase-timing.jsonl') as admission:
    with admission.phase('frozen_member_recheck'):
        for member in frozen['members'].values():
            assert hashlib.sha256(Path(member['path']).read_bytes()).hexdigest() == member['sha256']
    with admission.phase('frozen_input_recheck'):
        for name, digest in json.loads((inputs / 'preregistration.json').read_text())['inputs'].items():
            assert hashlib.sha256((inputs / name).read_bytes()).hexdigest() == digest

env = os.environ.copy()
output_limit = registered.get('outputTokenLimit', 4096)
assert type(output_limit) is int and output_limit in (4096, 8192)
env['ASTER_LH_OUTPUT_TOKENS'] = str(output_limit)
env.update(ASTER_CONFIG_FILE=str(root / '.aster/config.acl'), ASTER_LIVE_MODEL='glm-01/glm-5.3', ASTER_LH_EVIDENCE_DIR=str(attempts), ASTER_LH_CANDIDATE_MANIFEST=str(out / (label + '-pair.json')), ASTER_DELOOP_HOLDOUT_INPUTS=str(inputs))
cases = [('holdout', 'deloop_generalization_real_llm', 'opaque_selected_records_survive_a_real_long_run'), ('feed', 'deloop_real_llm', 'changing_external_batches_survive_compaction_in_one_real_acp_run')]
assert registered['originalCases'] == [name for name, _, _ in cases]
rows = []
for name, member, test in cases:
    prior_traces = set(attempts.glob('*/phase-timing.jsonl'))
    with Recorder(attempts / (name + '-phase-timing.jsonl')) as timing:
        with timing.phase('config_identity_recheck'):
            assert hashlib.sha256((root / '.aster/config.acl').read_bytes()).hexdigest() == registered['configSha256']
        command = [frozen['members'][member]['path'], '--ignored', '--exact', test, '--nocapture', '--test-threads=1']
        row = {'case': name, 'attempt': 1, 'status': 'RUNNING', 'startedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'command': command}
        rows.append(row)
        (attempts / 'attempt-status.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(json.dumps({'case': name, 'label': label, 'status': 'RUNNING'}), flush=True)
        started = time.monotonic()
        result = timing.run_logged(command, cwd=root, env=env, log=attempts / (name + '-attempt-1.log'), phase='test_process')
        row.update(status='FINISHED', exitCode=result.returncode, outerSeconds=time.monotonic()-started, finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat())
    native_traces = sorted(set(attempts.glob('*/phase-timing.jsonl')) - prior_traces)
    summary = summarize([timing.path, *native_traces])
    if not native_traces:
        summary['complete'] = False
        summary['errors'].append({'kind': 'missing_native_trace'})
    process = next(point for point in summary['points'] if point['phase'] == 'process_started')
    row['timing'] = reconcile(summary, process['subject']['process'])
    required = ['campaign_prepare', 'config_load', 'freeze_pair', 'build_id_query', 'initialize', 'session_new', 'prompt', 'driver_close', 'relay_close', 'oracle', 'timing_flush']
    row['phaseCoverage'] = {phase: 'observed' if phase in summary['phases'] else 'not_observed' for phase in required}
    baseline = {}
    if len(sys.argv) > 2:
        baseline_path = out / (sys.argv[2] + '-live') / (name + '-phase-summary.json')
        if baseline_path.exists():
            previous = json.loads(baseline_path.read_text())
            baseline = {phase: value['union_us'] for phase, value in previous['phases'].items()}
    row['phaseDeltas'] = compare({phase: value['union_us'] for phase, value in summary['phases'].items()}, baseline)
    (attempts / (name + '-phase-summary.json')).write_text(json.dumps(summary, indent=2) + '\n')
    (attempts / 'attempt-status.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps({key: row[key] for key in ['case', 'status', 'exitCode', 'outerSeconds', 'timing']}), flush=True)

(attempts / 'phase-summary.json').write_text(json.dumps(summarize(sorted(attempts.glob('*phase-timing.jsonl')) + sorted(attempts.glob('*/phase-timing.jsonl'))), indent=2) + '\n')
raise SystemExit(1 if any(row['exitCode'] != 0 for row in rows) else 0)
