from pathlib import Path
import hashlib
import json
import os
import stat
import subprocess
import sys

root = Path.cwd()
sys.path.insert(0, str(root / 'crates/code/acp/tests/long_horizon'))
from timing_runner import Recorder
out = root / 'docs/evidence/code/2026-11-02-summary-input-retention'
timing = Recorder(out / (sys.argv[1] + '-phase-timing.jsonl'))
import atexit
atexit.register(timing.close)
result = timing.run_capture(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--',
    'apps/aster', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml',
    'scripts/cli_pair_build.rs'], cwd=root, phase='source_paths')
result.check_returncode()
paths = result.stdout.split(b'\0')
with timing.phase('source_read_hash'):
    hasher = subprocess.Popen(['git', 'hash-object', '--stdin'], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    records = {}
    for name in sorted(set(p for p in paths if p)):
        path = root / os.fsdecode(name)
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(mode):
            data = os.fsencode(os.readlink(path))
        elif stat.S_ISREG(mode):
            data = path.read_bytes()
        else:
            continue
        hasher.stdin.write(name + b'\0' + len(data).to_bytes(8, 'little') + data)
        records[os.fsdecode(name)] = hashlib.sha256(data).hexdigest()
    hasher.stdin.close()
    digest = hasher.stdout.read().decode().strip()
    assert hasher.wait() == 0
record = {'scope': 'Complete CLI source namespace from scripts/cli_pair_build.rs, including dirty/untracked source; no credentials copied',
          'gitSourceHash': digest, 'paths': records}
path = out / (sys.argv[1] + '-namespace.json')
assert not path.exists(), path
path.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'path': str(path), 'sourcePaths': len(records), 'gitSourceHash': digest}), flush=True)
timing.close()
