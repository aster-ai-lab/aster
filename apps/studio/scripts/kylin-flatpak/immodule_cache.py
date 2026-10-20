"""Rewrite a GTK immodules cache onto the Flatpak module path."""

import argparse
import re
import sys
from pathlib import Path

MODULE_ID = re.compile(r"[A-Za-z0-9_-]+")
SANDBOX_MODULE = "/app/lib/im/fcitx5/im-fcitx5.so"


def _quoted(path):
    if path == "" or "\n" in path or '"' in path:
        raise ValueError("path must be one line without quotes")
    return f'"{path}"'


def module_id_from_cache(text, query_path):
    """Return the module id on the descriptor line after query_path."""
    target = _quoted(query_path)
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.strip() != target:
            continue
        if index + 1 >= len(lines):
            raise ValueError("immodule descriptor missing")
        match = re.match(r'"([A-Za-z0-9_-]+)"', lines[index + 1].strip())
        if match is None:
            raise ValueError("immodule id missing")
        return match.group(1)
    raise ValueError("query path missing from immodule cache")


def rewrite_immodule_cache(text, query_path, sandbox_path, module_id):
    """Point the cache at sandbox_path and require module_id."""
    if MODULE_ID.fullmatch(module_id) is None:
        raise ValueError("module id must be one token")
    if module_id_from_cache(text, query_path) != module_id:
        raise ValueError("module id does not match cache")
    old = _quoted(query_path)
    new = _quoted(sandbox_path)
    rewritten = text.replace(old, new)
    if query_path != sandbox_path and old in rewritten:
        raise ValueError("query path remained in cache")
    if new not in rewritten:
        raise ValueError("sandbox path missing from cache")
    if f'"{module_id}"' not in rewritten:
        raise ValueError("module id missing from cache")
    return rewritten


def main(argv):
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    identify = sub.add_parser("id")
    identify.add_argument("--cache", required=True)
    identify.add_argument("--query-path", required=True)

    rewrite = sub.add_parser("rewrite")
    rewrite.add_argument("--cache", required=True)
    rewrite.add_argument("--query-path", required=True)
    rewrite.add_argument("--module-id", required=True)
    rewrite.add_argument("--sandbox-path", default=SANDBOX_MODULE)
    rewrite.add_argument("--out", required=True)

    args = parser.parse_args(argv)
    cache_text = Path(args.cache).read_text()
    if args.command == "id":
        sys.stdout.write(module_id_from_cache(cache_text, args.query_path) + "\n")
        return 0
    rewritten = rewrite_immodule_cache(
        cache_text, args.query_path, args.sandbox_path, args.module_id
    )
    destination = Path(args.out)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(rewritten)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except ValueError as error:
        print(f"immodule cache: {error}", file=sys.stderr)
        sys.exit(1)
