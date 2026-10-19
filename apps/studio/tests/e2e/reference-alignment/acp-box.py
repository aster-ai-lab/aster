#!/usr/bin/env python3
"""Test-only stdio transport; the Studio host still owns the actual ACP turn."""

import os
import hashlib
from pathlib import Path
import subprocess
import shlex
import shutil
import sys


def main():
    cli = os.environ["A3S_STUDIO_REFERENCE_BOX_CLI"]
    box = os.environ["A3S_STUDIO_REFERENCE_BOX"]
    args = sys.argv[1:]
    config = Path(args[args.index("--config") + 1]).resolve(strict=True)
    job = Path(os.environ["A3S_STUDIO_REFERENCE_JOB"]).resolve(strict=True)
    if config.parent != job / "state" / "runtime":
        raise ValueError("refuse config outside this job")
    if "--plugin-store" in args:
        store = Path(args[args.index("--plugin-store") + 1]).resolve(strict=True)
        if store != job / "package-generations" or not (job / "readonly-review").is_file():
            raise ValueError("refuse plugin store outside this review job")
        archive = Path(shutil.make_archive(str(job / "review-publication"), "gztar", root_dir=store))
        subprocess.run([cli, "cp", str(archive), f"{box}:/tmp/review-publication.tar.gz"], check=True, stdout=sys.stderr)
        uploaded = subprocess.check_output([cli, "exec", box, "--", "sha256sum", "/tmp/review-publication.tar.gz"])
        if uploaded.decode().split()[0] != hashlib.sha256(archive.read_bytes()).hexdigest():
            raise ValueError("guest review publication differs from host publication")
        subprocess.run([cli, "exec", box, "--", "mkdir", "-p", str(store)], check=True, stdout=sys.stderr)
        subprocess.run([cli, "exec", box, "--", "tar", "-xzf", "/tmp/review-publication.tar.gz", "-C", str(store)], check=True, stdout=sys.stderr)
        subprocess.run([cli, "exec", box, "--", "chown", "-R", str(os.getuid()), str(store)], check=True, stdout=sys.stderr)
    subprocess.run([cli, "exec", box, "--", "mkdir", "-p", str(config.parent)], check=True, stdout=sys.stderr)
    subprocess.run([cli, "cp", str(config), f"{box}:{config}"], check=True, stdout=sys.stderr)
    subprocess.run([cli, "exec", box, "--", "chown", str(os.getuid()), str(config)], check=True, stdout=sys.stderr)
    subprocess.run([cli, "exec", box, "--", "chmod", "600", str(config)], check=True, stdout=sys.stderr)
    ssh = shutil.which("ssh")
    if not ssh:
        raise ValueError("OpenSSH client is required")
    os.execv(ssh, [ssh, "-T", "-p", os.environ["A3S_STUDIO_REFERENCE_SSH_PORT"],
                   "-i", str(job / "ssh-key"), "-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes",
                   "-o", "StrictHostKeyChecking=yes", "-o", f"UserKnownHostsFile={job / 'known-hosts'}",
                   "reference-runner@127.0.0.1", shlex.join(["/opt/reference-agent/a3s-code-acp", *args])])


if __name__ == "__main__":
    main()
