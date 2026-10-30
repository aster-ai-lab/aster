"""Freeze a compiled live harness beside its native product pair."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("platform", choices=("windows", "linux"))
parser.add_argument("phase")
parser.add_argument("binary", type=Path)
parser.add_argument("--expected-tests", type=int, required=True)
args = parser.parse_args()
campaign = Path(__file__).resolve().parent.parent
audit = campaign / args.platform / args.phase
manifest = json.loads(Path((audit / "manifest-path.txt").read_text().strip()).read_text())
dest = Path(manifest["folder"]) / args.binary.name
assert not dest.exists()
shutil.copy2(args.binary, dest)
inventory = subprocess.check_output([str(dest), "--list", "--ignored"], text=True)
names = [line.removesuffix(": test") for line in inventory.splitlines() if line.endswith(": test")]
assert len(names) == len(set(names)) == args.expected_tests
(audit / "test-inventory.txt").write_text(inventory, encoding="utf-8", newline="\n")
sha = hashlib.sha256()
with dest.open("rb") as stream:
    magic = stream.read(4)
    assert magic.startswith(b"MZ") if args.platform == "windows" else magic == b"\x7fELF"
    stream.seek(0)
    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
        sha.update(chunk)
metadata = {"source_pair": manifest["source_pair"], "binary": str(dest),
            "sha256": sha.hexdigest(), "bytes": dest.stat().st_size,
            "entrypoint_count": len(names), "entrypoints": names}
(audit / "test-binary.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({"source_pair": metadata["source_pair"], "entrypoint_count": len(names), "test_binary": str(dest)}))
