#!/usr/bin/env python3
"""Generate and check the Aster desktop update channel manifest.

`generate` writes `desktop-latest.json` for one platform archive and
refuses to regress an existing newer (or equal) version unless --force.
`check` validates the manifest contract that the studio host updater
(`apps/studio/src-tauri/src/app_updater.rs`) pins. Every refusal names
its reason on stderr and exits nonzero.
"""

import argparse
import hashlib
import json
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path

PRODUCT = "aster"
CHANNEL = "stable"
REPO = "aster-ai-lab/aster"
CHANNEL_TAG = "desktop-stable"
MAX_SIZE = 512 * 1024 * 1024


def parse_version(text):
    parts = text.split(".")
    if len(parts) != 3 or any(not part.isascii() or not part.isdigit() for part in parts):
        return None
    try:
        return tuple(int(part) for part in parts)
    except ValueError:
        return None


def digest_of(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def load(path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"manifest unreadable: {path}: {error}")


def generate(args):
    version = parse_version(args.version)
    if version is None:
        raise SystemExit(f"version must be x.y.z integers, got {args.version!r}")
    archive = Path(args.zip)
    if not archive.is_file():
        raise SystemExit(f"archive missing: {archive}")
    size = archive.stat().st_size
    if size == 0 or size > MAX_SIZE:
        raise SystemExit(f"archive size out of range: {size}")
    if args.platform != "darwin-aarch64" or args.asset != f"Aster-darwin-aarch64-v{args.version}.zip":
        raise SystemExit("desktop asset name must match its supported platform and version")
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "apps/studio/scripts"))
    from studio_delivery import verify_archive
    try:
        verify_archive(archive, args.version, PRODUCT)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        raise SystemExit(f"invalid desktop archive: {error}")
    if args.existing:
        current = load(args.existing)
        existing_version = parse_version(str(current.get("version", "")))
        if existing_version is None:
            raise SystemExit(f"existing manifest version unparsable: {current.get('version')!r}")
        if existing_version >= version and not args.force:
            relation = "equal to" if existing_version == version else "newer than"
            raise SystemExit(
                f"refusing: existing published version {'.'.join(map(str, existing_version))} is {relation} {args.version}"
            )
    notes = Path(args.notes).read_text(encoding="utf-8") if args.notes else ""
    manifest = {
        "product": PRODUCT,
        "channel": CHANNEL,
        "version": args.version,
        "published_at": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "notes": notes,
        "platforms": {
            args.platform: {
                "url": f"https://github.com/{REPO}/releases/download/{CHANNEL_TAG}/{args.asset}",
                "sha256": digest_of(archive),
                "size": size,
            }
        },
    }
    Path(args.out).write_text(
        json.dumps(manifest, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    print(f"wrote {args.out} version {args.version} platform {args.platform}")


def check(args):
    manifest = load(args.manifest)
    if manifest.get("product") != PRODUCT:
        raise SystemExit(f"product must be {PRODUCT!r}, got {manifest.get('product')!r}")
    if manifest.get("channel") != CHANNEL:
        raise SystemExit(f"channel must be {CHANNEL!r}, got {manifest.get('channel')!r}")
    if parse_version(str(manifest.get("version", ""))) is None:
        raise SystemExit(f"version must be x.y.z integers, got {manifest.get('version')!r}")
    if not str(manifest.get("published_at", "")).startswith("20"):
        raise SystemExit(f"published_at missing or malformed: {manifest.get('published_at')!r}")
    if not isinstance(manifest.get("notes"), str):
        raise SystemExit("notes must be a string")
    platforms = manifest.get("platforms")
    if not isinstance(platforms, dict) or not platforms:
        raise SystemExit("platforms must be a non-empty object")
    for platform, entry in platforms.items():
        if platform != "darwin-aarch64":
            raise SystemExit(f"unsupported desktop platform: {platform}")
        url = entry.get("url", "")
        expected_prefix = f"https://github.com/{REPO}/releases/download/{CHANNEL_TAG}/"
        if not url.startswith(expected_prefix):
            raise SystemExit(f"{platform}: url must start with {expected_prefix}, got {url!r}")
        digest = entry.get("sha256", "")
        if len(digest) != 64 or any(
            character not in "0123456789abcdef" for character in digest
        ):
            raise SystemExit(f"{platform}: sha256 must be 64 lowercase hex characters")
        size = entry.get("size")
        if not isinstance(size, int) or isinstance(size, bool) or not 0 < size <= MAX_SIZE:
            raise SystemExit(f"{platform}: size must be an integer in (0, {MAX_SIZE}]")
    print(f"manifest ok: version {manifest['version']} platforms {sorted(platforms)}")


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    gen = sub.add_parser("generate")
    gen.add_argument("--zip", required=True)
    gen.add_argument("--asset", required=True, help="release asset name the url names")
    gen.add_argument("--version", required=True)
    gen.add_argument("--platform", default="darwin-aarch64")
    gen.add_argument("--notes")
    gen.add_argument("--out", required=True)
    gen.add_argument("--existing", help="current published manifest for regression refusal")
    gen.add_argument("--force", action="store_true")
    gen.set_defaults(func=generate)
    chk = sub.add_parser("check")
    chk.add_argument("--manifest", required=True)
    chk.set_defaults(func=check)
    args = parser.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main(sys.argv[1:])
