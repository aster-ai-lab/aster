"""Keep temporary download credentials outside the shared deployment workspace."""

import json
import os
from pathlib import Path
import stat

PROXY_PORTS = (18095, 18096, 18097, 18098)


def private_directory():
    directory = Path.home()/'.aster-h200-xq-download'
    if not directory.exists():
        return None
    metadata = directory.stat()
    if directory.is_symlink() or metadata.st_uid != os.getuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
        raise RuntimeError('download credential directory ownership or permissions are invalid')
    return directory


def credential_file(name):
    directory = private_directory()
    if directory is None:
        return None
    path = directory/name
    metadata = path.stat()
    if (path.is_symlink() or not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600 or metadata.st_size > 2048):
        raise RuntimeError('download credential file ownership, permissions, or size are invalid')
    return path


def curl_environment(proxy):
    environment = dict(os.environ)
    if proxy not in {f'socks5h://127.0.0.1:{port}' for port in PROXY_PORTS}:
        return environment
    path = credential_file('.curlrc')
    if path is not None:
        environment['CURL_HOME'] = str(path.parent)
    return environment


def socks_authentication():
    path = credential_file('auth.json')
    if path is None:
        return None
    value = json.loads(path.read_text())
    username, password = value.get('username'), value.get('password')
    if (username != 'aster-download' or not isinstance(password, str) or len(password) != 64
            or any(character not in '0123456789abcdef' for character in password)):
        raise RuntimeError('download authentication record has an invalid format')
    return username.encode('ascii'), password.encode('ascii')
