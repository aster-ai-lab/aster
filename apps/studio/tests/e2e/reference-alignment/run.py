#!/usr/bin/env python3
"""Run original task graders after a real Studio -> ACP -> Boyue turn."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import socket
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[5]
TEXT_REVIEW_TASKS = ("review-text-basic", "review-text-clean")
REVIEW_TASKS = ("review-basic", "review-clean", "review-official", "review-security", "review-unrelated", "review-mixed", "review-ambiguous", "review-selected-b") + TEXT_REVIEW_TASKS
QUESTION_TASKS = ("studio-question-choice", "studio-question-text")
EXPECTED_TESTS = {"regex-log": 1, "git-leak-recovery": 5, "studio-tables": 4, "studio-content": 8, "studio-proofreading": 4, "studio-web": 4, **dict.fromkeys(REVIEW_TASKS + QUESTION_TASKS, 1)}
TASKS = tuple(EXPECTED_TESTS)
LIVE_TEST = "agent_plane::reference_tasks_live_tests::reference_task_boyue_live"


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def credential_env(path):
    env = os.environ.copy()
    if env.get("BOYUE_API_KEY"):
        return env
    for line in path.read_text().splitlines():
        name, separator, value = line.strip().removeprefix("export ").partition("=")
        if separator and name == "BOYUE_API_KEY":
            env[name] = "".join(shlex.split(value))
            break
    if not env.get("BOYUE_API_KEY"):
        raise ValueError("BOYUE_API_KEY is required; no alternate model is permitted")
    return env


def original_tests_passed(task, report):
    expected = EXPECTED_TESTS[task]
    results = report.get("results", {})
    summary = results.get("summary", {})
    tests = results.get("tests", [])
    return (summary.get("tests") == expected and summary.get("passed") == expected
            and all(summary.get(key) == 0 for key in ("failed", "skipped", "pending", "other"))
            and len(tests) == expected and all(test.get("status") == "passed" for test in tests))


def remove_reasoning(report):
    for message in report.get("messages", []):
        data = message.get("data", {})
        if "parts" in data:
            data["parts"] = [part for part in data["parts"] if part.get("type") != "reasoning"]


def copy_inputs(source, workspace):
    inputs = source / "inputs"
    if inputs.is_symlink() or (inputs.exists() and not inputs.is_dir()):
        raise ValueError("fixture inputs must be a real directory")
    if not inputs.exists():
        return {}
    for path in inputs.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("fixture inputs must contain only regular files and directories")
    shutil.copytree(inputs, workspace / "inputs")
    return {str(path.relative_to(inputs)): sha256(path) for path in inputs.rglob("*") if path.is_file()}


def pin_verifier_inputs(source, job):
    pinned = job / "verifier-inputs"
    pinned.mkdir()
    for name in ("test.sh", "test_outputs.py", "expected.json"):
        path = source / "tests" / name
        if name == "expected.json" and not path.exists():
            continue
        if path.is_symlink() or not path.is_file():
            raise ValueError("verifier inputs must be regular source files")
        shutil.copyfile(path, pinned / name)
    return pinned


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", choices=TASKS, required=True)
    parser.add_argument("--native-test-bin", type=Path, required=True)
    parser.add_argument("--linux-acp", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=ROOT.parent / "tests")
    parser.add_argument("--box-cli", type=Path, required=True)
    parser.add_argument("--image", default="a3s-studio-reference:ssh-ubuntu24.04")
    parser.add_argument("--verifier-image", help="prepared image with the original verifier's pinned tool cache")
    options = parser.parse_args()
    cli = str(options.box_cli.resolve(strict=True))
    native = options.native_test_bin.resolve(strict=True)
    agent = options.linux_acp.resolve(strict=True)
    for executable in (Path(cli), native, agent):
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise ValueError(f"not an executable file: {executable}")
    with agent.open("rb") as stream:
        if stream.read(4) != b"\x7fELF":
            raise ValueError("Linux ACP must be an ELF executable")
    source = options.source.resolve(strict=True) / options.task
    env = credential_env(ROOT / "scripts/harbor/.env")
    ssh, scp, keygen = (shutil.which(name) for name in ("ssh", "scp", "ssh-keygen"))
    if not all((ssh, scp, keygen)):
        raise ValueError("OpenSSH client tools are required")
    uid = os.getuid()
    if uid == 0:
        raise ValueError("run the native reference lane as a non-root host user")
    job = Path(tempfile.mkdtemp(prefix=f"a3s-studio-reference-{options.task}-")).resolve()
    os.chmod(job, 0o700)
    (job / ".owned").touch()
    pinned_native = job / "native-test"
    shutil.copyfile(native, pinned_native)
    os.chmod(pinned_native, 0o500)
    workspace = job / "workspace"
    workspace.mkdir()
    inputs = copy_inputs(source, workspace)
    wrapper = job / "acp-box.py"
    shutil.copyfile(Path(__file__).with_name("acp-box.py"), wrapper)
    os.chmod(wrapper, 0o700)
    shutil.copyfile(source / "instruction.md", job / "instruction.txt")
    verifier_inputs = pin_verifier_inputs(source, job)
    if options.task in REVIEW_TASKS:
        (job / "readonly-review").touch()
    if options.task in TEXT_REVIEW_TASKS:
        (job / "text-review").touch()
    if options.task == "review-ambiguous":
        (job / "expect-clarification").touch()
    if options.task in QUESTION_TASKS or options.task == "review-selected-b":
        shutil.copyfile(source / "tests/reply.json", job / "answer-question.json")
    name = "studio-reference-" + job.name.rsplit("-", 1)[-1]
    verifier = name + "-verifier"
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    env.update(A3S_STUDIO_REFERENCE_JOB=str(job), A3S_STUDIO_REFERENCE_ACP=str(wrapper),
               A3S_STUDIO_REFERENCE_AGENT_ARTIFACT=str(agent), A3S_STUDIO_REFERENCE_BOX=name,
               A3S_STUDIO_REFERENCE_BOX_CLI=cli)
    env["A3S_STUDIO_REFERENCE_SSH_PORT"] = str(port)
    result = {"schema": "a3s.studio.reference-task.v1", "task": options.task,
              "model": "boyue/deepseek-v4-flash", "status": "infrastructure-error",
              "sourceInstructionSha256": sha256(job / "instruction.txt"),
              "originalGraderSha256": sha256(verifier_inputs / "test_outputs.py"),
              "verifierSourceSha256": {path.name: sha256(path) for path in verifier_inputs.iterdir()},
              "nativeTestSha256": sha256(pinned_native), "linuxAcpSha256": sha256(agent),
              "resources": {"cpus": 1, "memoryGiB": 2, "diskQuotaEnforced": False, "agentUid": uid},
              "separateVerifierVm": True,
              "solutionProvidedToAgent": False, "nativeExit": None, "graderReward": None}
    result["graderProvenance"] = "original-reference" if options.task in ("regex-log", "git-leak-recovery") else "synthetic-feature-fixture"
    result["acceptanceScope"] = "native typed question and explicit user reply through ACP and Boyue; not a Tauri GUI claim" if options.task in QUESTION_TASKS else "controlled local package publication and actual Word review host; no signed registry, product installation or native GUI claim" if options.task in REVIEW_TASKS else {
        "regex-log": "native-task-and-original-grader",
        "git-leak-recovery": "native-task-and-original-grader",
        "studio-tables": "synthetic-data-transform; no native GUI or scanned OCR",
        "studio-content": "synthetic-source-fact checks; requires prose review and native GUI",
        "studio-proofreading": "explicit-prompt behavior; no installed review-plugin or native GUI claim",
        "studio-web": "artifact structure preflight; requires real browser interaction",
    }[options.task]
    if options.task == "review-ambiguous":
        result["acceptanceScope"] = "typed clarification guard with no selected document; requires-input is not completion; no user answer, continuation, signed publication or GUI claim"
    result["inputSha256"] = inputs
    print(f"job={job}", flush=True)
    started = time.monotonic()
    created = False
    verifier_created = False
    with (job / "infrastructure.log").open("w") as log:
        def box(*args, check=True, timeout=940, capture=False):
            return subprocess.run([cli, *map(str, args)], check=check, timeout=timeout,
                                  stdout=subprocess.PIPE if capture else log, stderr=log)
        try:
            box("run", "-d", "--name", name, "--cpus", "1", "--memory", "2g",
                "-v", f"{workspace}:{workspace}", "--publish", f"{port}:22",
                options.image, "--", "sleep", "3600")
            created = True
            box("exec", name, "--", "/usr/sbin/useradd", "--create-home", "--uid", uid, "reference-runner")
            subprocess.run([keygen, "-q", "-t", "ed25519", "-N", "", "-f", str(job / "ssh-key")],
                           stdout=log, stderr=log, check=True)
            box("exec", name, "--", "mkdir", "-p", "/opt/reference-agent", "/home/reference-runner/.ssh", "/run/sshd")
            box("exec", name, "--", "chown", uid, "/opt/reference-agent")
            box("cp", job / "ssh-key.pub", f"{name}:/home/reference-runner/.ssh/authorized_keys")
            box("exec", name, "--", "chown", "-R", uid, "/home/reference-runner/.ssh")
            box("exec", name, "--", "chmod", "700", "/home/reference-runner/.ssh")
            box("exec", name, "--", "chmod", "600", "/home/reference-runner/.ssh/authorized_keys")
            box("exec", name, "--", "ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", "/tmp/reference-ssh-host-key")
            box("cp", f"{name}:/tmp/reference-ssh-host-key.pub", job / "ssh-host-key.pub")
            (job / "known-hosts").write_text(f"[127.0.0.1]:{port} " + (job / "ssh-host-key.pub").read_text())
            box("exec", name, "--", "/usr/sbin/sshd", "-p", "22", "-o", "HostKey=/tmp/reference-ssh-host-key",
                "-o", "PermitRootLogin=no", "-o", "PasswordAuthentication=no",
                "-o", "KbdInteractiveAuthentication=no", "-o", "AllowTcpForwarding=no",
                "-o", "PermitTunnel=no", "-o", "X11Forwarding=no")
            ssh_options = ["-i", str(job / "ssh-key"), "-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes",
                           "-o", "StrictHostKeyChecking=yes", "-o", f"UserKnownHostsFile={job / 'known-hosts'}"]
            subprocess.run([scp, "-P", str(port), *ssh_options, str(agent),
                            "reference-runner@127.0.0.1:/opt/reference-agent/a3s-code-acp"],
                           stdout=log, stderr=log, check=True, timeout=120)
            uploaded = box("exec", name, "--", "sha256sum", "/opt/reference-agent/a3s-code-acp", capture=True)
            if uploaded.stdout.decode().split()[0] != result["linuxAcpSha256"]:
                raise ValueError("guest ACP bytes differ from the hashed binary")
            box("exec", name, "--", "chown", "-R", "0:0", "/opt/reference-agent")
            box("exec", name, "--", "chmod", "555", "/opt/reference-agent/a3s-code-acp")
            box("exec", name, "--", "ln", "-s", workspace, "/app")
            if options.task == "git-leak-recovery":
                box("cp", source / "environment/challenge-setup.sh", f"{name}:/tmp/challenge-setup.sh")
                box("exec", "--workdir", "/app", name, "--", "bash", "/tmp/challenge-setup.sh")
                box("exec", name, "--", "rm", "/tmp/challenge-setup.sh")
                box("exec", name, "--", "chown", "-R", uid, "/app/repo")
            with (job / "native-test.log").open("w") as native_log:
                run = subprocess.run([str(pinned_native), "--ignored", "--exact", LIVE_TEST,
                                      "--nocapture", "--test-threads=1"], env=env,
                                     stdout=native_log, stderr=subprocess.STDOUT, timeout=930)
            result["nativeExit"] = run.returncode
            turn = json.loads((job / "turn-result.json").read_text())
            result["nativeModelConfirmed"] = (
                turn.get("model") == "boyue/deepseek-v4-flash"
                and turn.get("nativeModel") == "boyue::deepseek-v4-flash"
                and turn.get("acpSha256") == result["linuxAcpSha256"]
                and turn.get("instructionSha256") == result["sourceInstructionSha256"]
            )
            result["guestCredentialCleanupExit"] = box(
                "exec", name, "--", "rm", "-rf", job / "state/runtime", check=False, timeout=15).returncode
            if result["guestCredentialCleanupExit"] != 0:
                raise RuntimeError("guest credential cleanup failed")
            box("rm", "-f", name, timeout=30)
            created = False
            result["cleanupExit"] = 0
            result["remoteAgentClosed"] = True
            if options.task in REVIEW_TASKS + QUESTION_TASKS:
                remove_reasoning(turn)
                (job / "grader-turn.json").write_text(json.dumps(turn, ensure_ascii=False))
            # Destroying the agent VM closes every process before graders enter.
            box("run", "-d", "--name", verifier, "--cpus", "1", "--memory", "2g",
                "-v", f"{workspace}:{workspace}", options.verifier_image or options.image, "--", "sleep", "3600")
            verifier_created = True
            box("exec", verifier, "--", "ln", "-s", workspace, "/app")
            if options.task == "git-leak-recovery":
                box("exec", verifier, "--", "env", "HOME=/root", "git", "config", "--global", "--add", "safe.directory", workspace / "repo")
            box("exec", verifier, "--", "mkdir", "-p", "/tests", "/logs/verifier")
            for filename in ("test.sh", "test_outputs.py"):
                box("cp", verifier_inputs / filename, f"{verifier}:/tests/{filename}")
            if options.task in REVIEW_TASKS + QUESTION_TASKS:
                box("cp", job / "grader-turn.json", f"{verifier}:/tests/turn-result.json")
                box("cp", verifier_inputs / "expected.json", f"{verifier}:/tests/expected.json")
            verification = box("exec", "--timeout", "900", "--workdir", "/app", verifier,
                               "--", "env", "HOME=/root", "bash", "/tests/test.sh", check=False)
            result["verifierExit"] = verification.returncode
            reward = box("exec", verifier, "--", "cat", "/logs/verifier/reward.txt", check=False, capture=True)
            result["graderReward"] = reward.stdout.decode().strip()
            (job / "verifier").mkdir()
            for filename in ("reward.txt", "ctrf.json"):
                box("cp", f"{verifier}:/logs/verifier/{filename}", job / "verifier" / filename)
            result["originalTestsPassed"] = original_tests_passed(options.task, json.loads((job / "verifier/ctrf.json").read_text()))
            result["inputsUnchanged"] = all((workspace / "inputs" / path).is_file() and sha256(workspace / "inputs" / path) == digest for path, digest in inputs.items())
            result["verifierInputsUnchanged"] = all(sha256(verifier_inputs / name) == digest and sha256(source / "tests" / name) == digest for name, digest in result["verifierSourceSha256"].items())
            assert result["verifierInputsUnchanged"]
            native_outcome = turn.get("status")
            result["scenarioOutcome"] = native_outcome
            accepted_outcome = native_outcome == "success" or (options.task == "review-ambiguous" and native_outcome == "requires-input")
            result["status"] = "grader-pass" if run.returncode == 0 and result["nativeModelConfirmed"] and accepted_outcome and verification.returncode == 0 and result["graderReward"] == "1" and result["originalTestsPassed"] and result["inputsUnchanged"] else "failed"
        except Exception as error:
            result["error"] = type(error).__name__
        finally:
            result["elapsedSeconds"] = round(time.monotonic() - started, 2)
            if created:
                try:
                    result["guestCredentialCleanupExit"] = box("exec", name, "--", "rm", "-rf", job / "state/runtime", check=False, timeout=15).returncode
                except (OSError, subprocess.TimeoutExpired):
                    result["guestCredentialCleanupExit"] = -1
                try:
                    result["cleanupExit"] = box("rm", "-f", name, check=False, timeout=30).returncode
                except (OSError, subprocess.TimeoutExpired):
                    result["cleanupExit"] = -1
            if verifier_created:
                try:
                    result["verifierCleanupExit"] = box("rm", "-f", verifier, check=False, timeout=30).returncode
                except (OSError, subprocess.TimeoutExpired):
                    result["verifierCleanupExit"] = -1
            shutil.rmtree(job / "state/runtime", ignore_errors=True)
            (job / "ssh-key").unlink(missing_ok=True)
            pinned_native.unlink(missing_ok=True)
            result["localCredentialsRemoved"] = not (job / "state/runtime").exists() and not (job / "ssh-key").exists()
            if (created and (result["cleanupExit"] != 0 or result.get("guestCredentialCleanupExit") != 0)) or result.get("verifierCleanupExit", 0) != 0 or not result["localCredentialsRemoved"]:
                result["status"] = "cleanup-failed"
            key = env["BOYUE_API_KEY"]
            log.flush()
            for path in [job / "infrastructure.log", job / "native-test.log"]:
                if path.exists():
                    path.write_text(path.read_text().replace(key, "[redacted]"))
            turn_path = job / "turn-result.json"
            if turn_path.exists():
                turn = json.loads(turn_path.read_text())
                remove_reasoning(turn)
                turn_path.write_text(json.dumps(turn, ensure_ascii=False, indent=2) + "\n")
            (job / "task-result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result.get(k) for k in ("task", "model", "status", "nativeExit", "graderReward", "cleanupExit")}))
    return 0 if result["status"] == "grader-pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
