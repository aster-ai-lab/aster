"""Freeze both native Cargo binary artifacts and record their shared build ID."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("platform", choices=("windows", "linux"))
    parser.add_argument("--phase", default="continuation-20261030-pair")
    args = parser.parse_args()
    root = Path.cwd().resolve()
    campaign = Path(__file__).resolve().parent.parent
    build = campaign / args.platform / args.phase / "build.jsonl"
    binaries = {}
    for line in build.read_text(encoding="utf-8-sig").splitlines():
        record = json.loads(line)
        if record.get("reason") != "compiler-artifact" or not record.get("executable"):
            continue
        name = record["target"]["name"]
        if name in ("aster", "a3s-code-acp"):
            binaries[name] = Path(record["executable"])
    assert set(binaries) == {"aster", "a3s-code-acp"}, "both Cargo binary artifacts required"
    identities = {
        subprocess.check_output([str(path), "--build-id"], text=True).strip()
        for path in binaries.values()
    }
    assert len(identities) == 1, "TUI/ACP build IDs differ"
    identity = identities.pop()
    native = "x86_64-pc-windows-msvc" if args.platform == "windows" else "x86_64-unknown-linux-gnu"
    assert identity.startswith("aster-cli-pair-v1:") and identity.endswith(":" + native)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S%f")
    folder = root / ".tmp" / ("paired-cli-" + args.platform) / (args.phase + "-" + stamp)
    folder.mkdir(parents=True)
    files = []
    for path in binaries.values():
        dest = folder / path.name
        shutil.copy2(path, dest)
        sha = hashlib.sha256()
        with dest.open("rb") as stream:
            magic = stream.read(4)
            assert magic.startswith(b"MZ") if args.platform == "windows" else magic == b"\x7fELF"
            stream.seek(0)
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                sha.update(chunk)
        assert subprocess.check_output([str(dest), "--build-id"], text=True).strip() == identity
        files.append({"name": dest.name, "bytes": dest.stat().st_size, "sha256": sha.hexdigest()})
    result = {
        "source_pair": identity,
        "folder": str(folder),
        "files": files,
        "build_json": str(build),
        "created_at": datetime.now(timezone.utc).isoformat(),
    }
    manifest = folder / "pair.json"
    manifest.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
    (campaign / args.platform / args.phase / "manifest-path.txt").write_text(
        str(manifest) + "\n", encoding="utf-8", newline="\n"
    )
    print(json.dumps({"manifest": str(manifest), "source_pair": identity}))


if __name__ == "__main__":
    main()
