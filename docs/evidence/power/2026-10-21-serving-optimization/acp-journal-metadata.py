#!/usr/bin/env python3
"""Read one bounded Gateway journal window and export typed metadata only."""
import argparse
from datetime import datetime, timezone, timedelta
import hashlib
import json
import subprocess
import threading
from urllib.parse import urlsplit
import uuid


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def access_metadata(row):
    raw = row.get('MESSAGE')
    if isinstance(raw, list) and len(raw) <= 65536 and all(type(x) is int and 0 <= x <= 255 for x in raw):
        raw = bytes(raw).decode('utf-8', 'replace')
    if not isinstance(raw, str) or len(raw) > 65536 or 'access_log' not in raw or '{' not in raw:
        return None
    try:
        value, _ = json.JSONDecoder().raw_decode(raw[raw.index('{'):])
    except (ValueError, TypeError):
        return None
    if not isinstance(value, dict) or not all(type(value.get(k)) is int and 0 <= value[k] < 2**64 for k in ('status', 'duration_ms', 'response_bytes')) or not 100 <= value['status'] <= 599:
        return None
    stamp, method, path = value.get('timestamp'), value.get('method'), value.get('path')
    if not all(isinstance(x, str) and len(x) <= 4096 for x in (stamp, method, path)):
        return None
    try:
        ended = datetime.fromisoformat(stamp.replace('Z', '+00:00'))
        if ended.tzinfo is None:
            return None
        started = ended - timedelta(milliseconds=value['duration_ms'])
    except (ValueError, OverflowError):
        return None
    known_paths = ('/v1/chat/completions', '/v1/models', '/health', '/metrics')
    result = {'completion_utc': ended.astimezone(timezone.utc).isoformat(),
              'estimated_start_utc': started.astimezone(timezone.utc).isoformat(),
              'method': method if method in ('GET', 'POST', 'DELETE', 'OPTIONS') else 'other',
              'path': path if path in known_paths else 'other',
              'http_status': value['status'], 'duration_ms': value['duration_ms'],
              'response_bytes': value['response_bytes'], 'request_id': None, 'power_index': None}
    if path not in known_paths:
        result['path_sha256'] = digest(path)
    inference = value.get('inference')
    if isinstance(inference, dict) and isinstance(inference.get('request_id'), str):
        try:
            result['request_id'] = str(uuid.UUID(inference['request_id']))
        except ValueError:
            pass
    backend = value.get('backend')
    if isinstance(backend, str) and len(backend) <= 4096:
        try:
            parsed = urlsplit(backend)
            if parsed.scheme == 'http' and parsed.hostname in ('127.0.0.1', 'localhost') and parsed.username is None and parsed.password is None and not parsed.query and not parsed.fragment and parsed.path in ('', '/') and parsed.port in range(18200, 18208):
                result['power_index'] = parsed.port - 18200
        except ValueError:
            pass
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--since', required=True)
    parser.add_argument('--until', required=True)
    args = parser.parse_args()
    since, until = (datetime.fromisoformat(x.replace('Z', '+00:00')) for x in (args.since, args.until))
    if since.tzinfo is None or until.tzinfo is None or not 0 < (until - since).total_seconds() <= 120:
        raise SystemExit('bounded UTC window required')
    command = ['journalctl', '-u', 'a3s-gateway.service', '--since', since.strftime('%Y-%m-%d %H:%M:%S UTC'),
               '--until', until.strftime('%Y-%m-%d %H:%M:%S UTC'), '-n', '500', '--no-pager', '-o', 'json']
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    watchdog = threading.Timer(12, child.kill)
    watchdog.start()
    try:
        raw = child.stdout.read(8 * 1024 * 1024 + 1)
        if len(raw) > 8 * 1024 * 1024:
            raise RuntimeError('journal byte bound exceeded')
        exit_code = child.wait(timeout=2)
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=2)
        child.stdout.close()
        watchdog.cancel()
        watchdog.join(timeout=1)
    rows, lines, malformed = [], raw.splitlines(), 0
    for line in lines:
        try:
            row = access_metadata(json.loads(line))
        except (ValueError, TypeError):
            malformed += 1
            continue
        if row is not None:
            rows.append(row)
    print(json.dumps({'command': command, 'exit_code': exit_code, 'journal_records': len(lines),
                      'bounded_window_saturated': len(lines) >= 500, 'malformed_records': malformed,
                      'access_events': rows, 'caller_attribution': 'UNKNOWN; no own HTTP request IDs observed',
                      'timestamp_semantics': 'entry timestamp at terminal access-log construction; estimated start subtracts duration, asynchronous emission can lag',
                      'raw_message_exported': False, 'credential_or_prompt_exported': False}))


if __name__ == '__main__':
    main()
