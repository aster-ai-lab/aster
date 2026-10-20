#!/usr/bin/python3
# Experiment only: Rosetta rejects PR_SET_SECCOMP. Retain every other Flatpak argument.
import json
import os
from pathlib import Path
import sys

ROOT = Path('/aster/target/evaluations/kylin-ime-hardening')
removed = []
opened = []

def rewrite(words):
    result = []
    index = 0
    while index < len(words):
        word = words[index]
        if word == '--seccomp':
            fd = words[index + 1]
            assert fd.isdecimal(), 'unexpected seccomp fd'
            removed.append(fd)
            index += 2
        elif word == '--args':
            fd = int(words[index + 1])
            payload = os.pread(fd, 1024 * 1024, 0)
            assert payload.endswith(b'\0'), 'unexpected bwrap argument stream'
            decoded = [os.fsdecode(value) for value in payload[:-1].split(b'\0')]
            revised = rewrite(decoded)
            next_fd = os.memfd_create('ime-experiment-bwrap-args', 0)
            os.write(next_fd, b'\0'.join(os.fsencode(value) for value in revised) + b'\0')
            os.lseek(next_fd, 0, os.SEEK_SET)
            os.set_inheritable(next_fd, True)
            opened.append(next_fd)
            result.extend(['--args', str(next_fd)])
            index += 2
        else:
            result.append(word)
            index += 1
    return result

def flatten(words):
    result = []
    index = 0
    while index < len(words):
        if words[index] == '--args':
            payload = os.pread(int(words[index + 1]), 1024 * 1024, 0)
            assert payload.endswith(b'\0')
            result.extend(flatten([os.fsdecode(value) for value in payload[:-1].split(b'\0')]))
            index += 2
        else:
            result.append(words[index])
            index += 1
    return result

before = flatten(sys.argv[1:])
args = rewrite(sys.argv[1:])
after = flatten(args)
expected = []
index = 0
while index < len(before):
    if before[index] == '--seccomp':
        index += 2
    else:
        expected.append(before[index])
        index += 1
assert after == expected, 'test wrapper changed a non-seccomp argument'
x11_binds = [before[index:index + 3] for index, value in enumerate(before)
             if value == '--ro-bind' and '/tmp/.X11-unix/X' in before[index + 1]]
with (Path('/aster/target/evaluations/kylin-ime-gui') / 'bwrap-experiment.jsonl').open('a') as output:
    output.write(json.dumps({'removed_only_seccomp_fds': removed, 'all_other_semantic_arguments_identical': after == expected, 'original_argument_count': len(before), 'forwarded_argument_count': len(after), 'x11_socket_binds': x11_binds, 'wrapper_environment_DISPLAY': os.environ.get('DISPLAY')}) + '\n')
os.execv(str(ROOT / 'flatpak16/usr/bin/bwrap'), ['bwrap', *args])
