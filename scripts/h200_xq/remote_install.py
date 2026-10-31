#!/usr/bin/env python3
"""Rebuild on an allocated H200 node, activate verified weights, and tune."""

import argparse
import gzip
import hashlib
import json
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from prepare_assets import (XOLOTL_SHA256, XOLOTL_REVISION, LLAMA_REVISION, digest,
                           MODEL_MANIFEST_SHA256, MODEL_TREE_SHA256,
                           MODEL_AUXILIARY_SHA256, TOKENIZER_SHA256)
from service import ROOT, STAGING, POWER_SHA256, require_hardware

ENGRAM_SHA256 = {
    1: '5cc4ca3be6302a50597684d234dd47ae7ff7b3a5b42d386515b58f4ecd17662b',
    14: '35c6bb1c15139a691d530c4ea469d1983031074c1a84b4e7f7668fef2015b68b',
}
SNAPSHOT = '/mnt/shared-storage-gpfs2/gpfs2-shared-public/huggingface/hub/models--deepseek-ai--DeepSeek-V4.1-Flash-1017/snapshots/1bae0e1088993b9b82de08f59de5be5a34e42170'
STAGING_SHA256 = {
    'source/xolotl.tar.gz': XOLOTL_SHA256,
    'source/llama-tree.json': '615b8d6e6f8614a337b579f9ef9fa54482abe6ac7dbf5d2389d187d8350ad768',
    'validated-h200-engine.tar.gz': 'c5574540f36a37deef52ec4f72acca894b738f97f72dfac2bef37c6becfb3c95',
    'assets/v3/SHA256SUMS': MODEL_MANIFEST_SHA256,
    'assets/v3-tree.json': MODEL_TREE_SHA256,
    **{'assets/v3/'+name: checksum for name, checksum in MODEL_AUXILIARY_SHA256.items()},
    **{'assets/tokenizer/'+name: checksum for name, checksum in TOKENIZER_SHA256.items()},
}


def checked_file(root, name, maximum=128*1024**2):
    path = root/name
    if (not path.resolve().is_relative_to(root.resolve()) or path.is_symlink()
            or not path.is_file() or path.stat().st_size > maximum):
        raise RuntimeError('missing, unsafe or oversized preparation artifact: '+name)
    return path


def verify_staging(root, software_root=None):
    """Check software and small assets without requiring or reading GGUF weights."""
    software_root = root if software_root is None else Path(software_root)
    if software_root.is_symlink() or not software_root.resolve().is_relative_to(root.resolve()):
        raise RuntimeError('prepared software must stay inside the owned staging root')
    artifacts = []
    for name, expected in STAGING_SHA256.items():
        path = checked_file(root, name)
        if digest(path) != expected:
            raise RuntimeError('preparation artifact checksum mismatch: '+name)
        artifacts.append({'file': name, 'bytes': path.stat().st_size, 'sha256': expected})
    power = hashlib.sha256()
    power_bytes = 0
    with gzip.open(checked_file(software_root, 'aster-power-glibc235.gz'), 'rb') as source:
        for block in iter(lambda: source.read(8*1024**2), b''):
            power_bytes += len(block)
            if power_bytes > 128*1024**2:
                raise RuntimeError('prepared Power binary exceeds its size limit')
            power.update(block)
    if power.hexdigest() != POWER_SHA256:
        raise RuntimeError('prepared Power binary checksum mismatch')
    ready = checked_file(root, 'source/SOURCES_READY', 1024)
    if ready.read_text().splitlines() != [XOLOTL_REVISION, LLAMA_REVISION]:
        raise RuntimeError('prepared source revisions do not match the pinned build')
    metadata = checked_file(root, 'source/llama-tree.json', 4*1024**2)
    tree = json.loads(metadata.read_text())
    if tree.get('truncated') or tree.get('sha') != LLAMA_REVISION:
        raise RuntimeError('prepared ggml tree is truncated or has the wrong identity')
    blobs = [item for item in tree['tree'] if item['type'] == 'blob' and item['path'].startswith('ggml/')]
    if not blobs or any(item['mode'] not in ('100644', '100755') for item in blobs):
        raise RuntimeError('prepared ggml source list is empty or has unsupported modes')
    for item in blobs:
        path = checked_file(root, 'source/llama/'+item['path'], 64*1024**2)
        size = path.stat().st_size
        if size != item['size']:
            raise RuntimeError('prepared ggml source size mismatch: '+item['path'])
        checksum = hashlib.sha1(f'blob {size}\0'.encode())
        with path.open('rb') as source:
            for block in iter(lambda: source.read(8*1024**2), b''):
                checksum.update(block)
        if checksum.hexdigest() != item['sha']:
            raise RuntimeError('prepared ggml Git blob mismatch: '+item['path'])
    manifest = json.loads(checked_file(software_root, 'operator-files.json', 65536).read_text())
    if not isinstance(manifest, list) or not 1 <= len(manifest) <= 64:
        raise RuntimeError('prepared operator manifest has an invalid size')
    names = set()
    for item in manifest:
        if (not isinstance(item, dict) or set(item) != {'file', 'sha256'}
                or not isinstance(item['file'], str) or Path(item['file']).name != item['file']
                or item['file'] in names):
            raise RuntimeError('prepared operator manifest has an invalid or duplicate entry')
        names.add(item['file'])
        if digest(checked_file(software_root, item['file'], 1024**2)) != item['sha256']:
            raise RuntimeError('prepared operator checksum mismatch: '+item['file'])
    return {'unix': time.time(), 'nonweight_ready': True, 'artifacts': artifacts,
            'power_binary_sha256': POWER_SHA256, 'power_binary_bytes': power_bytes,
            'software_bundle': str(software_root.resolve().relative_to(root.resolve())),
            'ggml_verified_files': len(blobs), 'operator_verified_files': len(names),
            'main_weights_checked': False, 'gpu_inference_tested': False}


def run_preflight():
    destination = STAGING/'logs/nonweight-preflight.json'
    destination.parent.mkdir(parents=True, exist_ok=True)
    def publish(record):
        temporary = destination.with_suffix('.tmp')
        temporary.write_text(json.dumps(record, indent=2))
        temporary.replace(destination)
    try:
        record = verify_staging(STAGING)
    except Exception as error:
        publish({'unix': time.time(), 'nonweight_ready': False,
                 'error_type': type(error).__name__, 'reason': str(error)})
        raise
    publish(record)
    return record


def run_autotune():
    try:
        subprocess.run([sys.executable, str(STAGING/'autotune.py')], check=True)
    finally:
        # Restore normal privacy even if the campaign process itself fails.
        # This action never reloads the Xolotl model or changes its profile.
        subprocess.run([sys.executable, str(STAGING/'service.py'), 'set-usage-mode'], check=True)


def main():
    run_preflight()
    require_hardware()
    mount = subprocess.check_output(['findmnt', '--target', '/nvme', '--noheadings', '--output', 'TARGET'], text=True).strip()
    if mount != '/nvme' or shutil.disk_usage('/nvme').free < 500 * 1024**3:
        raise RuntimeError('deployment requires the host NVMe mount with at least 500 GiB free')
    for directory in ('logs', 'models', 'engram-a', 'engram-b', 'source/xolotl', 'run', 'bench', 'bin'):
        (ROOT/directory).mkdir(parents=True, exist_ok=True)
    source = STAGING/'source/xolotl.tar.gz'
    if digest(source) != XOLOTL_SHA256:
        raise RuntimeError('Xolotl source archive failed the pinned SHA-256 gate')
    subprocess.run(['tar', '--no-same-owner', '--strip-components=1', '-xzf', str(source),
                    '-C', str(ROOT/'source/xolotl')], check=True)
    power = ROOT/'bin/aster-power'
    with gzip.open(STAGING/'aster-power-glibc235.gz', 'rb') as compressed, power.open('wb') as output:
        shutil.copyfileobj(compressed, output)
    if digest(power) != POWER_SHA256:
        raise RuntimeError('Power binary failed the pinned SHA-256 gate')
    power.chmod(0o755)
    subprocess.run([str(power), '--version'], check=True)
    extractor = ROOT/'source/xolotl/tools/v41/extract_engram.py'
    if digest(extractor) != 'f14c1bc89749d301b1074197aeb16669d1b62acb414f31eca952f8a177bb8432':
        raise RuntimeError('Engram extractor failed the pinned SHA-256 gate')
    extracts = []
    for layer, directory in [(1, 'engram-a'), (14, 'engram-b')]:
        existing = ROOT/directory/f'engram-L{layer}.safetensors'
        if existing.is_file():
            if digest(existing) != ENGRAM_SHA256[layer] or not (existing.parent/'model.safetensors.index.json').is_file():
                raise RuntimeError('existing Engram data is incomplete or has a different identity')
            continue
        log = (ROOT/f'logs/extract-{layer}.log').open('ab', buffering=0)
        extracts.append(subprocess.Popen([sys.executable, str(extractor), SNAPSHOT,
                                          '--to', f'layers.{layer}={ROOT/directory}'],
                                         stdout=log, stderr=subprocess.STDOUT))
        log.close()
    with (ROOT/'logs/build-engine.log').open('ab', buffering=0) as log:
        build = subprocess.run(['bash', str(STAGING/'build_engine.sh')], stdout=log, stderr=subprocess.STDOUT)
    statuses = [process.wait() for process in extracts]
    if build.returncode or any(statuses):
        raise RuntimeError(f'preparation failed: build={build.returncode}, extracts={statuses}')
    for layer, directory in [(1, 'engram-a'), (14, 'engram-b')]:
        if digest(ROOT/directory/f'engram-L{layer}.safetensors') != ENGRAM_SHA256[layer]:
            raise RuntimeError('extracted Engram data failed its pinned SHA-256 gate')
    subprocess.run([sys.executable, str(STAGING/'frontend_smoke.py')], check=True)
    subprocess.run([sys.executable, str(STAGING/'bootstrap.py')], check=True)
    run_autotune()
    campaign = json.loads((ROOT/'bench/campaign.json').read_text())
    if (campaign.get('complete') is not True or campaign.get('long_context_qualified') is not True
            or campaign['best_warm_median_output_tokens_per_second'] < 0):
        raise RuntimeError('no inference profile passed qualification')
    if json.loads((ROOT/'run/active-profile.json').read_text()).get('exact_benchmark_usage') is not False:
        raise RuntimeError('qualified service has not restored normal privacy')
    for filename in ('campaign.json',):
        shutil.copy2(ROOT/'bench'/filename, STAGING/'logs'/filename)
    print('QUALIFIED_H200_SERVICE_RUNNING', flush=True)
    while True:
        time.sleep(60)
        with urllib.request.urlopen('http://127.0.0.1:8150/health', timeout=10) as response:
            if json.load(response).get('loaded') is not True:
                raise RuntimeError('upstream engine stopped being loaded')
        with urllib.request.urlopen('http://127.0.0.1:30002/health', timeout=10) as response:
            if response.status != 200:
                raise RuntimeError('Power frontend stopped being healthy')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight', action='store_true', help='verify prepared nonweight artifacts without GPUs')
    args = parser.parse_args()
    if args.preflight:
        print(json.dumps(run_preflight()), flush=True)
    else:
        main()
