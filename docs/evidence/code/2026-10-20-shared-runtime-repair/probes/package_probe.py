"""Verify the actual local two-binary archive without extracting executables."""
import hashlib
import json
from pathlib import Path
import sys
import tarfile

archive = Path(sys.argv[1])
hashes = {}
data = {}
names = []
with tarfile.open(archive, 'r|gz') as package:
    for member in package:
        assert member.name in {'aster', 'a3s-code-acp', 'BUILD_ID', 'SHA256SUMS'}, member.name
        assert member.isfile(), member.name
        names.append(member.name)
        stream = package.extractfile(member)
        if member.name in {'BUILD_ID', 'SHA256SUMS'}:
            assert member.size <= 1024
            data[member.name] = stream.read(1025).decode()
        else:
            digest = hashlib.sha256()
            while chunk := stream.read(1024 * 1024):
                digest.update(chunk)
            hashes[member.name] = digest.hexdigest()
assert sorted(names) == sorted(['aster', 'a3s-code-acp', 'BUILD_ID', 'SHA256SUMS'])
expected = ''.join(f'{hashes[name]}  {name}\n' for name in ('aster', 'a3s-code-acp'))
assert data['SHA256SUMS'] == expected
assert data['BUILD_ID'].startswith('aster-cli-pair-v1:')
print(json.dumps({'members':names, 'build_id':data['BUILD_ID'].strip(),
    'sha256':hashes,'checksums_verified':True}, indent=2))
