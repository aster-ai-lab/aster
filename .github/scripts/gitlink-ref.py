"""Resolve only an exact commit gitlink from the checked-out root revision."""

import subprocess
import sys


def gitlink_ref(path):
    record = subprocess.check_output(
        ["git", "ls-tree", "-z", "HEAD", "--", path], text=True
    )
    entries = record.removesuffix("\0").split("\0")
    if len(entries) != 1 or "\t" not in entries[0]:
        raise ValueError(f"Expected one recorded gitlink: {path}")
    header, recorded_path = entries[0].split("\t", 1)
    fields = header.split()
    if (len(fields) != 3 or fields[:2] != ["160000", "commit"]
            or recorded_path != path or len(fields[2]) != 40
            or any(char not in "0123456789abcdef" for char in fields[2])):
        raise ValueError(f"Expected an exact commit gitlink: {path}")
    return fields[2]


if __name__ == "__main__":
    print(f"ref={gitlink_ref(sys.argv[1])}")
