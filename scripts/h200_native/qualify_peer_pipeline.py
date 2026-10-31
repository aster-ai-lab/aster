"""Qualify a frozen native executable inside an already verified owned H200 lease."""
import csv
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import time


EXPECTED_TESTS = {
    'cuda::tests::gpu::h200_explicit_scalar_profile_preserves_checkpoint_subnormal_product',
    'cuda::tests::gpu::h200_raw_group32_mma_matches_independent_scalar_inputs',
    'cuda::xq::tests::gpu::h200_xq_all_packed_formats_match_independent_golden_row_dots',
    'cuda::xq::tests::gpu::h200_xq_gather_preserves_distinct_repeated_nonordered_rows_and_every_golden_bit',
    'layers::xq::text::tests::gpu::h200_xq_embedding_shifted_f32_mhc_and_output_head_match_independent_math',
    'layers::xq::moe::tests::gpu::h200_xq_mixed_experts_and_shifted_ffn_match_independent_f64_math',
    'layers::xq::attention::tests::gpu::h200_xq_f32_projections_codecs_rope_and_sparse_attention_match_stage_oracles',
    'layers::xq::indexer::tests::gpu::h200_xq_f32_compression_index_projections_and_atomic_cache_match_stage_oracles',
    'layers::xq::engram::tests::gpu::h200_xq_engram_packed_lookup_f16_projection_f32_gate_and_carried_mix_match_oracles',
    'weights::xq::load::tests::gpu::h200_xq_loaded_source_consumer_f32_attention_ffn_and_atomic_prefix_match_chunked_execution',
    'execution::selection::tests::gpu::h200_xq_greedy_ties_choose_first_global_token_in_strided_last_row',
    'parallel::tests::gpu::h200x8_xq_peer_snapshots_preserve_view_bits_and_request_local_cache_affinity',
    'parallel::tests::gpu::h200x2_xq_replica_local_peer_cache_preserves_bits_and_step_isolation',
}
PEER_TEST = 'parallel::tests::gpu::h200x8_xq_peer_snapshots_preserve_view_bits_and_request_local_cache_affinity'
PAIR_TEST = 'parallel::tests::gpu::h200x2_xq_replica_local_peer_cache_preserves_bits_and_step_isolation'


def qualification_profile(name):
    if name == 'xq-control-v17':
        return set(EXPECTED_TESTS), 431
    if name in ('xq-fused-sinkhorn-validation-v24', 'xq-private-clock-fused-v27',
                'xq-private-clock-control-v28'):
        return EXPECTED_TESTS | {
            'cuda::sinkhorn::tests::gpu::h200_xq_sinkhorn_fused_matches_f64_tensor_control_and_reports_alternating_cost',
            'cuda::finite::tests::gpu::h200_fused_finite_matches_tensor_control_for_strides_tail_bf16_and_invalid_values',
        }, 444 if name == 'xq-fused-sinkhorn-validation-v24' else 447
    if name in ('xq-validation-cache-v29', 'xq-validation-cache-control-v30'):
        old, _ = qualification_profile('xq-private-clock-fused-v27')
        return old | {
            'execution::validation_cache::tests::gpu::h200_step_validation_cache_preserves_predicates_views_step_isolation_and_reports_cost',
        }, 451
    if name in ('xq-activation-codecs-v31', 'xq-activation-codecs-control-v32',
                'xq-activation-codecs-v33', 'xq-activation-codecs-control-v34'):
        old, _ = qualification_profile('xq-validation-cache-v29')
        return old | {
            'cuda::activation::tests::gpu::h200_fused_activation_codecs_match_every_byte_midpoint_stride_and_report_cost',
        }, 452
    if name in ('xq-readonly-inputs-v35', 'xq-readonly-inputs-control-v36'):
        old, _ = qualification_profile('xq-activation-codecs-v33')
        return old, 454
    raise RuntimeError('unknown frozen native kernel qualification profile')


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        while chunk := source.read(1024**2):
            value.update(chunk)
    return value.hexdigest()


def main():
    root = Path(__file__).absolute().parent
    if root.resolve() != root or root.stat().st_uid != os.getuid() or os.getuid() != 10250:
        raise RuntimeError('qualification requires the exact owned private stage and UID')
    os.umask(0o077)
    result = dict(schema='aster-xq-peer-native-hardware-qualification-v1', unix=time.time(),
                  phase='preflight', complete_model_execution_claimed=False,
                  performance_claimed=False, cases={})
    deadline = time.monotonic() + 900

    def interrupted(number, _frame):
        raise RuntimeError('owned hardware qualification interrupted by signal ' + str(number))

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)

    def publish():
        temporary = root/'qualification-result.tmp'
        temporary.write_text(json.dumps(result, indent=2)+'\n')
        temporary.replace(root/'qualification-result.json')

    def capture(name, args, timeout, env=None):
        started = time.time()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError('qualification reached its frozen 900-second total budget')
        with (root/(name+'.stdout.log')).open('x') as stdout, (root/(name+'.stderr.log')).open('x') as stderr:
            child = subprocess.Popen(args, stdout=stdout, stderr=stderr, env=env)
            try:
                child.wait(timeout=min(timeout, remaining))
            finally:
                if child.poll() is None:
                    child.kill()
                child.wait()
        result['cases'][name] = dict(command=args, exit=child.returncode,
                                    elapsed_seconds=time.time()-started)
        publish()
        path = root/(name+'.stdout.log')
        if path.stat().st_size > 4*1024**2:
            raise RuntimeError('qualification stdout exceeds read bound')
        return child.returncode, path.read_text()

    try:
        manifest_path = root/'qualification-inputs.json'
        if manifest_path.stat().st_size > 65536:
            raise RuntimeError('qualification manifest exceeds read bound')
        manifest = json.loads(manifest_path.read_text())
        expected_tests, expected_controls = qualification_profile(manifest.get('kernel_profile', 'xq-control-v17'))
        if manifest['scope'] != 'native SM90/XQ component and eight-device peer tests; not full model':
            raise RuntimeError('unknown native qualification scope')
        mode = manifest.get('mode', 'hardware')
        if mode not in ('hardware', 'cpu_controls'):
            raise RuntimeError('native qualification mode requires hardware or cpu_controls')
        if manifest['worker_sha256'] != digest(Path(__file__)):
            raise RuntimeError('qualification worker source identity changed')
        proof_path = Path(manifest['resource_proof'])
        if proof_path.stat().st_size > 65536:
            raise RuntimeError('allocation proof exceeds read bound')
        proof = json.loads(proof_path.read_text())
        if proof['phase'] != 'passed' or proof['expectedGpus'] != 8 or proof['hostname'] != socket.gethostname():
            raise RuntimeError('qualification requires the verified eight-H200 allocation')
        if proof['leaseEndsUnix']-time.time() < 900:
            raise RuntimeError('allocation has less than the bounded qualification deadline remaining')
        runtime_paths = [path for path in os.environ.get('LD_LIBRARY_PATH', '').split(':') if path]
        if any('stubs' in Path(path).parts for path in runtime_paths):
            raise RuntimeError('CUDA linker stubs cannot qualify native execution')
        os.environ['LD_LIBRARY_PATH'] = ':'.join(['/usr/local/cuda/lib64']+runtime_paths)
        inventory = subprocess.run(['nvidia-smi', '--query-gpu=index,name,uuid,memory.total',
                                    '--format=csv,noheader,nounits'], capture_output=True,
                                   text=True, check=True, timeout=15).stdout
        rows = list(csv.reader(io.StringIO(inventory)))
        expected_ids = [gpu['uuid'] for gpu in proof['gpus']]
        if len(rows) != 8 or {row[2].strip() for row in rows} != set(expected_ids):
            raise RuntimeError('visible physical GPU identities changed since allocation proof')
        if any(row[1].strip() != 'NVIDIA H200' or int(row[3]) < 128*1024 for row in rows):
            raise RuntimeError('visible GPU memory/model differs from the admitted H200 profile')
        active = subprocess.run(['nvidia-smi', '--query-compute-apps=pid,gpu_uuid',
                                 '--format=csv,noheader,nounits'], capture_output=True,
                                text=True, check=True, timeout=15).stdout
        if active.strip():
            raise RuntimeError('owned allocation is already executing a CUDA workload')
        result.update(inputs=manifest, gpu_uuids=expected_ids)
        artifact = manifest['test_binary']
        archive = root/'aster-deepseek-cuda-tests.gz'
        if not 0 < artifact['archive_bytes'] <= 256*1024**2 or archive.stat().st_size != artifact['archive_bytes']:
            raise RuntimeError('qualification archive has an unbounded or changed size')
        if digest(archive) != artifact['archive_sha256']:
            raise RuntimeError('qualification archive identity changed')
        if not 0 < artifact['binary_bytes'] <= 1024**3:
            raise RuntimeError('qualification executable exceeds byte bound')
        work = Path('/tmp')/root.name
        work.mkdir(mode=0o700, exist_ok=False)
        binary = work/'aster-deepseek-cuda-tests'
        count = 0
        with gzip.open(archive, 'rb') as source, binary.open('xb') as target:
            while chunk := source.read(1024**2):
                count += len(chunk)
                if count > artifact['binary_bytes']:
                    raise RuntimeError('qualification archive expands beyond pinned size')
                target.write(chunk)
        if count != artifact['binary_bytes'] or digest(binary) != artifact['binary_sha256']:
            raise RuntimeError('qualification executable identity changed')
        binary.chmod(0o700)
        status, dependencies = capture('dynamic_dependencies', ['ldd', str(binary)], 15)
        if status or 'not found' in dependencies or '/stubs/' in dependencies:
            raise RuntimeError('qualification requires real CUDA runtime dependencies')
        status, listed = capture('ignored_inventory', [str(binary), '--ignored', '--list'], 30)
        names = [line.removesuffix(': test') for line in listed.splitlines() if line.endswith(': test')]
        if status or len(names) != len(expected_tests) or set(names) != expected_tests:
            raise RuntimeError('qualification executable has a different hardware test inventory')
        status, controls = capture('hermetic_controls', [str(binary), '--test-threads=1'], 180)
        expected_summary = f'test result: ok. {expected_controls} passed; 0 failed; {len(expected_tests)} ignored;'
        if status or expected_summary not in controls:
            raise RuntimeError('Linux CUDA-feature hermetic controls failed or counts changed')
        if mode == 'cpu_controls':
            result.update(phase='passed', hermetic_tests_passed=expected_controls,
                          hardware_tests_passed=0, completed_unix=time.time())
            publish()
            return
        result.update(phase='hardware_running', hermetic_tests_passed=expected_controls)
        publish()
        order = [(PEER_TEST, expected_ids)]
        order += [(PAIR_TEST, expected_ids[start:start+2]) for start in range(0, 8, 2)]
        order += [(name, [expected_ids[0]]) for name in sorted(expected_tests-{PEER_TEST, PAIR_TEST})]
        for index, (name, visible_ids) in enumerate(order):
            environment = dict(os.environ)
            environment['CUDA_VISIBLE_DEVICES'] = ','.join(visible_ids)
            command = [str(binary), name, '--exact', '--ignored', '--nocapture', '--test-threads=1']
            case = 'hardware_'+str(index).zfill(2)
            result['current_test'] = name
            publish()
            status, output = capture(case, command, 180, environment)
            result['cases'][case]['test'] = name
            result['cases'][case]['visible_gpu_uuids'] = visible_ids
            if status or 'test result: ok. 1 passed; 0 failed; 0 ignored;' not in output:
                # Reproduce the distinct failure alone before interpreting it.
                capture(case+'_isolated_recheck', command, 180, environment)
                raise RuntimeError('native hardware case failed: '+name)
            result['cases'][case]['passed'] = True
            publish()
        result.update(phase='passed', hardware_tests_passed=len(order), unique_hardware_tests_passed=len(expected_tests),
                      two_gpu_pair_tests_passed=4,
                      eight_gpu_peer_tests_passed=1, single_gpu_component_tests_passed=len(expected_tests)-2,
                      completed_unix=time.time())
        publish()
    except Exception as error:
        result.update(phase='failed', error_type=type(error).__name__, error=str(error),
                      completed_unix=time.time())
        publish()
        raise
    finally:
        print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
