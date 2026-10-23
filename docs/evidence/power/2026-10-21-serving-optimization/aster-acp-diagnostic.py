#!/usr/bin/env python3
"""Observe owned ACP protocol/TCP only; keep the original client/driver unchanged."""
import argparse
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import socket
import struct
import subprocess
import threading
import time
from unittest import mock

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('frozen_acp_0226', HERE / 'aster-acp-two-turn.py')
a = importlib.util.module_from_spec(spec)
spec.loader.exec_module(a)
FROZEN_SHA = '0226f997432f2aa9575ae0b79630df9a20e55bb788cb8ef279f672e7e9e7ddba'
STATES = {1: 'Closed', 2: 'Listen', 3: 'SynSent', 4: 'SynReceived', 5: 'Established',
          6: 'FinWait1', 7: 'FinWait2', 8: 'CloseWait', 9: 'Closing', 10: 'LastAck',
          11: 'TimeWait', 12: 'DeleteTcb'}


def request_metadata(raw, key):
    """Synthetic fixture evidence only: never export text, headers or arbitrary fields."""
    if len(raw) > 1024 * 1024 or any(value in raw for value in a.secret_variants(key)):
        raise a.Failure('unsafe or oversized mock request metadata')
    value = json.loads(raw)
    if not isinstance(value, dict) or not isinstance(value.get('messages'), list) or len(value['messages']) > 256:
        raise a.Failure('bounded mock messages required')
    def scalar(item, depth=0):
        if item is None or type(item) is bool or type(item) is int and abs(item) < 2**64:
            return item
        if type(item) is float and math.isfinite(item):
            return item
        if isinstance(item, str):
            return {'type': 'string', 'chars': len(item), 'sha256': a.digest(item.encode())}
        if depth < 2 and isinstance(item, list) and len(item) <= 16:
            return [scalar(row, depth + 1) for row in item]
        if depth < 2 and isinstance(item, dict) and len(item) <= 16:
            known = ('enabled', 'type', 'budget_tokens', 'max_tokens', 'effort', 'exclude')
            return {'known': {name: scalar(item[name], depth + 1) for name in known if name in item},
                    'other_field_count': sum(name not in known for name in item)}
        return {'type': 'unsupported'}
    rows, counts = [], {}
    for message in value['messages']:
        if not isinstance(message, dict) or message.get('role') not in ('system', 'user', 'assistant', 'tool', 'developer'):
            raise a.Failure('unexpected mock message role shape')
        role, content = message['role'], message.get('content')
        counts[role] = counts.get(role, 0) + 1
        rows.append({'role': role, 'content_type': 'string' if isinstance(content, str) else 'array' if isinstance(content, list) else 'null' if content is None else 'other',
                     'content_chars': len(content) if isinstance(content, str) else None,
                     'content_items': len(content) if isinstance(content, list) else None,
                     'tool_calls_count': len(message['tool_calls']) if isinstance(message.get('tool_calls'), list) else 0})
    names = ('max_tokens', 'max_completion_tokens', 'stream', 'temperature', 'top_p', 'thinking',
             'enable_thinking', 'reasoning', 'reasoning_effort', 'stop')
    return {'body_sha256': a.digest(raw), 'body_bytes': len(raw), 'message_count': len(rows),
            'role_counts': counts, 'messages': rows, 'tools_count': len(value['tools']) if isinstance(value.get('tools'), list) else 0,
            'known_options': {name: scalar(value[name]) for name in names if name in value}}


def tcp_rows(raw, pid, target):
    if not 4 <= len(raw) <= 1024 * 1024:
        raise a.Failure('IPHelper table byte bound differs')
    count = struct.unpack_from('<I', raw)[0]
    if 4 + count * 24 > len(raw):
        raise a.Failure('IPHelper row bound differs')
    result = []
    for index in range(count):
        state, local_ip, local_port, remote_ip, remote_port, owner = struct.unpack_from('<6I', raw, 4 + index * 24)
        if owner != pid:
            continue
        host, port = socket.inet_ntoa(struct.pack('<I', remote_ip)), socket.ntohs(remote_port & 65535)
        row = {'state': STATES.get(state, 'Other'), 'local_port': socket.ntohs(local_port & 65535)}
        if (host, port) == target:
            row.update(remote_class='public-gateway', remote_port=port)
        else:
            row.update(remote_class='other-loopback' if host.startswith('127.') else 'other',
                       remote_endpoint_sha256=a.digest((host + ':' + str(port)).encode()))
        result.append(row)
    return result


def owned_tcp(child, target):
    """IPHelper IPv4 owner table; only a still-live exact Popen process is accepted."""
    if child.poll() is not None:
        return []
    table = ctypes.WinDLL('iphlpapi').GetExtendedTcpTable
    table.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.ULONG), wintypes.BOOL,
                      wintypes.ULONG, wintypes.ULONG, wintypes.ULONG]
    table.restype = wintypes.DWORD
    size = wintypes.ULONG(0)
    code = table(None, ctypes.byref(size), False, socket.AF_INET, 5, 0)
    if code not in (0, 122) or not 4 <= size.value <= 1024 * 1024:
        raise a.Failure('bounded IPHelper size observation failed')
    buffer = ctypes.create_string_buffer(size.value)
    if table(buffer, ctypes.byref(size), False, socket.AF_INET, 5, 0) != 0:
        raise a.Failure('IPHelper owner observation failed')
    result = tcp_rows(buffer.raw[:size.value], child.pid, target)
    return result if child.poll() is None else []


def creation_ticks(child):
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    times = [wintypes.FILETIME() for _ in range(4)]
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    if not kernel.GetProcessTimes(wintypes.HANDLE(int(child._handle)), *[ctypes.byref(v) for v in times]):
        raise a.Failure('owned ACP process creation identity unavailable')
    return (times[0].dwHighDateTime << 32) | times[0].dwLowDateTime


class Observer:
    def __init__(self, key):
        self.key, self.started, self.lock = key, time.monotonic(), threading.Lock()
        self.events, self.errors, self.tcp, self.children, self.threads = [], [], [], [], []
        self.turn, self.pending, self.checkpoints = 0, {}, set()
        self.text = {number: {'deltas': 0, 'chars': 0, 'meaningful_deltas': 0,
                             'sha': hashlib.sha256(), 'first_at_ms': None} for number in (0, 1, 2)}

    def event(self, **fields):
        if len(self.events) >= 2048:
            raise a.Failure('bounded protocol observation exhausted')
        self.events.append(dict(at_ms=1000 * (time.monotonic() - self.started), **fields))

    def error(self, exc):
        with self.lock:
            if len(self.errors) < 16:
                self.errors.append({'exception': type(exc).__name__})

    def observe_safely(self, raw, incoming):
        try:
            self.observe(raw, incoming)
        except Exception as exc:
            self.error(exc)

    def observe(self, raw, incoming):
        with self.lock:
            if any(needle in raw for needle in a.secret_variants(self.key)):
                self.event(kind='credential-marker-detected')
                return
            try:
                value = json.loads(raw)
            except (ValueError, UnicodeError):
                self.event(kind='non-json-protocol', bytes=len(raw))
                return
            if not isinstance(value, dict):
                self.event(kind='non-object-protocol')
                return
            method, identifier = value.get('method'), value.get('id')
            if not incoming and method in ('initialize', 'session/new', 'session/prompt'):
                if method == 'session/prompt':
                    self.turn = identifier - 2 if identifier in (3, 4) else 0
                    self.pending[identifier] = time.monotonic()
                self.event(kind='client-send', method=method, id=identifier if type(identifier) is int else None)
            elif incoming and method == 'session/update':
                update = value.get('params', {}).get('update', {})
                kind = update.get('sessionUpdate')
                safe = kind if kind in ('agent_message_chunk', 'agent_thought_chunk', 'tool_call', 'tool_call_update',
                                         'plan', 'available_commands_update', 'current_mode_update', 'usage_update',
                                         'session_info_update', 'config_option_update') else 'other'
                self.event(kind='notification', notification=safe, turn=self.turn)
                if kind == 'agent_message_chunk':
                    content = update.get('content', {})
                    text = content.get('text') if content.get('type') == 'text' else None
                    if isinstance(text, str):
                        stats = self.text[self.turn]
                        stats['deltas'] += 1
                        stats['chars'] += len(text)
                        stats['meaningful_deltas'] += bool(text.strip())
                        stats['sha'].update(text.encode())
                        if text.strip() and stats['first_at_ms'] is None:
                            stats['first_at_ms'] = 1000 * (time.monotonic() - self.started)
            elif incoming and type(identifier) is int and identifier in (1, 2, 3, 4):
                started = self.pending.pop(identifier, None)
                elapsed = None if started is None else 1000 * (time.monotonic() - started)
                self.event(kind='response', id=identifier, has_error='error' in value,
                           prompt_elapsed_ms=elapsed, completed_within30seconds=None if elapsed is None else elapsed <= 30000)
            else:
                self.event(kind='other-protocol', has_method=method is not None, has_id='id' in value)

    def follow(self, child, target):
        try:
            generation = creation_ticks(child)
            with self.lock:
                self.event(kind='owned-child', pid=child.pid, creation_filetime=generation,
                           observed_utc=datetime.now(timezone.utc).isoformat())
            while child.poll() is None:
                if creation_ticks(child) != generation:
                    raise a.Failure('owned ACP process generation changed')
                rows = owned_tcp(child, target)
                with self.lock:
                    if len(self.tcp) >= 1000:
                        raise a.Failure('bounded TCP observation exhausted')
                    self.tcp.append({'at_ms': 1000 * (time.monotonic() - self.started), 'pid': child.pid,
                                     'creation_filetime': generation, 'connections': rows})
                    for identifier, started in self.pending.items():
                        if time.monotonic() - started >= 30 and identifier not in self.checkpoints:
                            self.checkpoints.add(identifier)
                            self.event(kind='prompt-incomplete-at30seconds', id=identifier,
                                       meaningful_deltas=self.text[self.turn]['meaningful_deltas'])
                time.sleep(.2)
        except Exception as exc:
            # An exited owned child is a normal end to observation. No PID is reopened.
            if child.poll() is None:
                self.error(exc)

    def pipe(self, original, incoming):
        observer = self
        class Pipe:
            def readline(self, *args):
                raw = original.readline(*args)
                if raw:
                    observer.observe_safely(raw, True)
                return raw
            def write(self, raw):
                observer.observe_safely(raw, False)
                return original.write(raw)
            def __getattr__(self, name):
                return getattr(original, name)
        return Pipe()

    def summary(self):
        with self.lock:
            return {'events': self.events, 'tcp_samples': self.tcp, 'observer_errors': self.errors,
                    'reader_semantics': 'observed at owned stdin/stdout I/O; not inferred from completed turns',
                    'text_by_turn': {number: {key: value for key, value in stats.items() if key != 'sha'}
                                     | {'output_sha256': stats['sha'].hexdigest()} for number, stats in self.text.items()}}


def run(root, url, key, turn_seconds=45, total_seconds=120):
    if a.digest(Path(a.__file__).read_bytes()) != FROZEN_SHA:
        raise a.Failure('original0226 driver differs')
    from urllib.parse import urlsplit
    parsed = urlsplit(url)
    target = (parsed.hostname, parsed.port or 80)
    observer, original = Observer(key), subprocess.Popen
    def spawn(*args, **kwargs):
        child = original(*args, **kwargs)
        # Always return an already-created child to the frozen cleanup owner,
        # including when optional observation setup fails.
        try:
            if list(args[0])[0] == str(a.BINARY):
                child.stdin = observer.pipe(child.stdin, False)
                child.stdout = observer.pipe(child.stdout, True)
                observer.children.append(child)
                thread = threading.Thread(target=observer.follow, args=(child, target), daemon=True)
                observer.threads.append(thread)
                thread.start()
        except Exception as exc:
            observer.error(exc)
        return child
    with mock.patch.object(subprocess, 'Popen', side_effect=spawn):
        result = a.dialogue(root, url, key, turn_seconds=turn_seconds, total_seconds=total_seconds)
    for thread in observer.threads:
        thread.join(timeout=3)
    result['diagnostic'] = observer.summary()
    result['original_driver_sha256'] = FROZEN_SHA
    if a.digest(Path(a.__file__).read_bytes()) != FROZEN_SHA:
        result.update(status='failed', original_driver_identity_changed=True)
    result['diagnostic_threads_terminated'] = all(not thread.is_alive() for thread in observer.threads)
    if not result['diagnostic_threads_terminated'] or observer.errors:
        result['diagnostic_status'] = 'unsupported'
    else:
        result['diagnostic_status'] = 'observed'
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--public', action='store_true', required=True)
    parser.add_argument('--tag', choices=('final-v4-diagnostic1',), required=True)
    args = parser.parse_args()
    target = HERE / ('aster-acp-' + args.tag + '.json')
    if target.exists():
        raise a.Failure('existing diagnostic evidence; no replacement or automatic retry')
    spec = importlib.util.spec_from_file_location('approved_public_credential_diagnostic', HERE / 'public-frontdoor-qualification.py')
    public = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(public)
    source_sha, reader_sha = a.digest(Path(__file__).read_bytes()), a.digest(Path(public.__file__).read_bytes())
    key = public.credential()
    result = run(a.REPO / '.git/optimization-20261021/acp' / args.tag, public.URL, key)
    if a.digest(Path(__file__).read_bytes()) != source_sha or a.digest(Path(public.__file__).read_bytes()) != reader_sha:
        result.update(status='failed', source_identity_changed=True)
    result.update(diagnostic_sha256=source_sha, credential_reader_sha256=reader_sha, url=public.URL)
    payload = (json.dumps(result, indent=2) + '\n').encode()
    if any(needle in payload for needle in a.secret_variants(key)):
        raise a.Failure('credential marker detected before diagnostic export')
    with target.open('xb') as stream:
        stream.write(payload)
    print(json.dumps({'file': target.name, 'status': result['status'], 'diagnostic_status': result['diagnostic_status'],
                      'turns': result['turns'], 'elapsed_seconds': result['elapsed_seconds']}))
    if result['status'] != 'passed':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
