import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(subprocess.check_output(['git','rev-parse','--show-toplevel'],text=True).strip())
RESULT_DIR = Path(os.environ['ASTER_REPAIR_PROBE_DIR']).resolve()
RESULT_DIR.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='aster-stdio-') as temporary:
    base = Path(temporary)
    workspace = base / 'workspace with spaces'
    workspace.mkdir()
    home = base / 'test-home'
    home.mkdir()
    config = base / 'fixture config.acl'
    config.write_text('default_model = "mock/a"\nproviders "mock" {\n base_url = "http://127.0.0.1:9/v1"\n api_key = "test-only"\n models "a" {}\n}\n')
    arguments = base / 'arguments.json'
    fake = base / 'fake-acp'
    fake.write_text('#!/usr/bin/env python3\nimport json,os,sys\nfrom pathlib import Path\nPath(os.environ["PAIR_ARGS_LOG"]).write_text(json.dumps(sys.argv[1:]))\n')
    fake.chmod(0o755)
    env = dict(os.environ, A3S_HOME=str(home), GROK_HOME=str(home), XDG_CONFIG_HOME=str(home),
        A3S_CONFIG=str(config), A3S_ACP_AGENT_BIN=str(fake), PAIR_ARGS_LOG=str(arguments))
    result = subprocess.run([str(ROOT/'target/debug/aster'), 'agent', '--model', 'mock/a',
        '--reasoning-effort', 'high', 'stdio'], cwd=workspace, env=env,
        stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=20)
    assert result.returncode == 0, result.stderr
    args = json.loads(arguments.read_text())
    expected = ['--reasoning-effort','high','--cwd',str(workspace.resolve()),'--model','mock/a','--config',str(config)]
    assert args == expected, args
    valid_env = dict(env)
    valid_env.pop('A3S_ACP_AGENT_BIN')
    valid = subprocess.run([str(ROOT/'target/debug/aster'), 'agent', '--model', 'mock/a', 'stdio'],
        cwd=workspace, env=valid_env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=20)
    assert valid.returncode == 0, valid.stderr
    # A shipped sibling is checked before starting the protocol.
    paired = base / 'installed'
    paired.mkdir()
    (paired/'aster').symlink_to(ROOT/'target/debug/aster')
    # current_exe resolves the real executable; copy it to exercise the sibling path.
    (paired/'aster').unlink()
    import shutil
    shutil.copy2(ROOT/'target/debug/aster', paired/'aster')
    mismatch = paired/'a3s-code-acp'
    mismatch.write_text('#!/bin/sh\nprintf "aster-cli-pair-v1:wrong-source:wrong-target\\n"\n')
    mismatch.chmod(0o755)
    env.pop('A3S_ACP_AGENT_BIN')
    refused = subprocess.run([str(paired/'aster'),'agent','stdio'], cwd=workspace, env=env,
        stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=20)
    assert refused.returncode != 0, refused.stdout
    assert 'different build identities' in refused.stderr, refused.stderr
    output = {'forwarded_options':True,'spaces_preserved':True,'mismatched_sibling_refused':True,'valid_shipped_pair_starts':True}
    (RESULT_DIR/'stdio-result.json').write_text(json.dumps(output, indent=2))
    print(json.dumps(output))
