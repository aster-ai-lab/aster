"""Independent held-out evaluation; --self-test is tool qualification, not model results."""

import argparse
import ast
import csv
import hashlib
import json
import math
import os
import pkgutil
import select
from pathlib import Path
import signal
import stat
import subprocess
import sys
import sysconfig
import tempfile
import time


ROOT = Path(__file__).resolve().parent
MAX_FILE = 64 * 1024 * 1024
TIMEOUT = 10
FRAME = "HELDOUT_RESULT="
SUPERVISOR = """import os, subprocess, sys
worker = subprocess.Popen([sys.executable, '-I', '-B', '-c', sys.argv[1]], stdin=subprocess.DEVNULL, close_fds=True)
status = worker.wait()
os.write(int(sys.argv[2]), str(status).encode('ascii'))
os.close(int(sys.argv[2]))
sys.stdin.buffer.read(1)
"""


def read(path, maximum=MAX_FILE):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > maximum:
        raise ValueError("regular bounded file required")
    with path.open("rb") as source:
        value = source.read(maximum + 1)
    if len(value) > maximum:
        raise ValueError("file grew past bound")
    return value


def sha(value):
    return hashlib.sha256(value).hexdigest()


def strict_json(value):
    def unique(pairs):
        result = {}
        for key, item in pairs:
            if key in result:
                raise ValueError("duplicate JSON key")
            result[key] = item
        return result

    def finite(value):
        result = float(value)
        if not math.isfinite(result):
            raise ValueError("non-finite JSON number")
        return result

    def reject(_):
        raise ValueError("non-finite JSON constant")

    return json.loads(value, object_pairs_hook=unique, parse_float=finite, parse_constant=reject)


def relative(root, name):
    if not isinstance(name, str) or "\\" in name or any(part in ("", ".", "..") for part in name.split("/")):
        raise ValueError("normal relative file path required")
    path = root / name
    if Path(name).is_absolute() or path.resolve(strict=True) != path:
        raise ValueError("symlink or non-canonical input refused")
    return path


def hashes(root):
    files = sorted(root.rglob("*"))
    if len(files) > 8192:
        raise ValueError("source tree bound exceeded")
    result = {}
    for path in files:
        if path.is_symlink():
            raise ValueError("source fixture symlink refused")
        if not path.is_dir():
            result[str(path.relative_to(root))] = sha(read(path))
    return result


def unchanged(path, original):
    try:
        return read(path) == original
    except (OSError, ValueError):
        return False


def check_hashes(root, frozen, excluded=()):
    if not isinstance(frozen, dict) or not frozen or len(frozen) > 8192:
        raise ValueError("nonempty frozen file map required")
    changed, total = [], 0
    for name, digest in frozen.items():
        if name in excluded:
            continue
        path = relative(root, name)
        total += path.lstat().st_size
        if total > 2 * 1024 * 1024 * 1024:
            raise ValueError("frozen file total bound exceeded")
        if not isinstance(digest, str) or sha(read(path)) != digest:
            changed.append(name)
    return {"matched": not changed, "checked": len(frozen) - len(set(frozen) & set(excluded)), "changed": changed[:32]}


def differences(actual, expected, path="$"):
    if type(actual) is not type(expected):
        return [f"{path}: wrong JSON type"]
    if isinstance(expected, dict):
        if set(actual) != set(expected):
            return [f"{path}: object keys differ"]
        return [error for key in expected for error in differences(actual[key], expected[key], f"{path}.{key}")][:32]
    if isinstance(expected, list):
        if len(actual) != len(expected):
            return [f"{path}: list length differs"]
        return [error for index, (left, right) in enumerate(zip(actual, expected)) for error in differences(left, right, f"{path}[{index}]")][:32]
    return [] if actual == expected else [f"{path}: value differs"]


def child_limits():
    import resource
    resource.setrlimit(resource.RLIMIT_FSIZE, (1024 * 1024, 1024 * 1024))
    resource.setrlimit(resource.RLIMIT_CPU, (5, 5))


def run_python(directory, code, timeout=TIMEOUT):
    started = time.monotonic()
    result = {"passed": False, "exit_code": None, "timed_out": False}
    if os.name != "posix":
        return {**result, "error": "bounded process-group execution not qualified on this platform", "seconds": 0}
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = None
        exit_read, exit_write = os.pipe()
        try:
            process = subprocess.Popen(
                [sys.executable, "-I", "-B", "-c", SUPERVISOR, code, str(exit_write)], cwd=directory,
                env={"PATH": os.defpath, "LANG": "C.UTF-8"},
                stdin=subprocess.PIPE, stdout=stdout, stderr=stderr,
                start_new_session=True, preexec_fn=child_limits, pass_fds=(exit_write,),
            )
            os.close(exit_write)
            exit_write = None
            # The trusted leader waits for our pipe after reaping its worker,
            # keeping this private group live and its identity anchored.
            if select.select([exit_read], [], [], timeout)[0]:
                status = os.read(exit_read, 64)
                if status:
                    result["exit_code"] = int(status)
                else:
                    result["error"] = "supervisor exited without worker status"
            else:
                result["timed_out"] = True
        except (OSError, subprocess.SubprocessError) as error:
            result["error"] = f"{type(error).__name__}: {str(error)[:1000]}"
        finally:
            os.close(exit_read)
            if exit_write is not None:
                os.close(exit_write)
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except OSError as error:
                    result["error"] = f"private-group cleanup errno {error.errno}"
                try:
                    process.wait(timeout=3)
                    result["direct_child_reaped"] = True
                except subprocess.TimeoutExpired:
                    result["direct_child_reaped"] = False
                    result["error"] = "process did not settle after group kill"
                result["supervisor_exit_code"] = process.returncode
                process.stdin.close()
        result["seconds"] = round(time.monotonic() - started, 6)
        for name, stream in (("stdout", stdout), ("stderr", stderr)):
            size = stream.seek(0, os.SEEK_END)
            stream.seek(max(0, size - 8192))
            result[name] = stream.read(8192).decode("utf-8", errors="replace")
            result[f"{name}_bytes"] = size
            result[f"{name}_truncated"] = size > 8192
        frames = [line[len(FRAME):] for line in result["stdout"].splitlines() if line.startswith(FRAME)]
        try:
            result["result"] = strict_json(frames[-1]) if len(frames) == 1 else None
        except (ValueError, UnicodeError):
            result["result"] = None
        result["passed"] = result["exit_code"] == 0 and not result["timed_out"] and result.get("direct_child_reaped") is True and "error" not in result and not result["stdout_truncated"] and isinstance(result["result"], dict) and result["result"].get("passed") is True
    return result


def static_imports(source):
    tree = ast.parse(source.decode("utf-8"))
    imports = sorted({name.name.split(".")[0] for node in ast.walk(tree) if isinstance(node, ast.Import) for name in node.names}
                     | {node.module.split(".")[0] for node in ast.walk(tree) if isinstance(node, ast.ImportFrom) and node.module})
    stdlib = set(getattr(sys, "stdlib_module_names", sys.builtin_module_names))
    if not hasattr(sys, "stdlib_module_names"):
        standard_path = Path(sysconfig.get_path("stdlib"))
        stdlib.update(module.name for module in pkgutil.iter_modules([str(standard_path), str(standard_path / "lib-dynload")]))
    return imports, sorted(set(imports) - stdlib)


def repair_cases(pricing, original_test, independent):
    results = []
    prefix = """import sys, os, json, inspect
sys.path.insert(0, os.getcwd())
def original_signature(function):
    parameters = list(inspect.signature(function).parameters.values())
    return [parameter.name for parameter in parameters] == ['prices', 'fee'] and all(parameter.kind == inspect.Parameter.POSITIONAL_OR_KEYWORD and parameter.default is inspect.Parameter.empty for parameter in parameters)
"""
    suite = prefix + """import unittest
suite = unittest.defaultTestLoader.loadTestsFromName('test_pricing')
result = unittest.TextTestRunner(verbosity=2).run(suite)
from pricing import total_price
signature_matches = original_signature(total_price)
passed = result.wasSuccessful() and result.testsRun == 3 and signature_matches
print('HELDOUT_RESULT=' + json.dumps({'passed': passed, 'tests_run': result.testsRun, 'failures': len(result.failures), 'errors': len(result.errors), 'signature_matches': signature_matches}))
sys.exit(0 if passed else 1)
"""
    codes = [("original_unittest_suite", suite)]
    for index, (prices, fee, expected) in enumerate(independent):
        codes.append((f"independent_{index + 1}", prefix + f"""from pricing import total_price
actual = total_price({prices!r}, {fee!r})
signature_matches = original_signature(total_price)
passed = type(actual) is int and actual == {expected!r} and signature_matches
print('HELDOUT_RESULT=' + json.dumps({{'passed': passed, 'actual': actual, 'expected': {expected!r}, 'actual_type': type(actual).__name__, 'signature_matches': signature_matches}}))
sys.exit(0 if passed else 1)
"""))
    with tempfile.TemporaryDirectory(prefix="heldout independent eval ") as temporary:
        for index, (name, code) in enumerate(codes):
            directory = Path(temporary) / str(index)
            directory.mkdir()
            (directory / "pricing.py").write_bytes(pricing)
            if index == 0:
                (directory / "test_pricing.py").write_bytes(original_test)
            result = {"case": name, **run_python(directory, code)}
            observed = result.get("result")
            if index == 0:
                result["payload_matches_case"] = isinstance(observed, dict) and set(observed) == {"passed", "tests_run", "failures", "errors", "signature_matches"} and type(observed["tests_run"]) is int and observed["tests_run"] == 3 and type(observed["failures"]) is int and observed["failures"] == 0 and type(observed["errors"]) is int and observed["errors"] == 0 and observed["signature_matches"] is True
            else:
                expected = independent[index - 1][2]
                result["payload_matches_case"] = isinstance(observed, dict) and set(observed) == {"passed", "actual", "expected", "actual_type", "signature_matches"} and type(observed["actual"]) is int and observed["actual"] == expected and type(observed["expected"]) is int and observed["expected"] == expected and observed["actual_type"] == "int" and observed["signature_matches"] is True
            result["isolated_submission_unchanged"] = unchanged(directory / "pricing.py", pricing)
            result["isolated_test_unchanged"] = index != 0 or unchanged(directory / "test_pricing.py", original_test)
            result["passed"] &= result["payload_matches_case"] and result["isolated_submission_unchanged"] and result["isolated_test_unchanged"]
            results.append(result)
    return results


def evaluate(case, run_path):
    started = time.monotonic()
    report = {"schema": "aster.heldout-evaluation.v1", "case": case, "passed": False, "artifact_passed": False}
    report["execution_boundary"] = "Repair uses Python -I -B, minimal environment and fresh temporary copies. Tested code shares the driver interpreter; this is functional evaluation, not an adversarial sandbox or authenticated child attestation. A trusted supervisor reports its worker's actual exit before private-group cleanup; the supervisor is then reaped. Timed-out worker exit is unknown when no status arrived. Detached descendants are not qualified."
    fixture = (ROOT / "heldout-inputs" / case).resolve()
    fixture_before = hashes(fixture)
    protected_before = {name: sha(read(ROOT / name)) for name in ("heldout-oracles.json", "heldout-selfcheck.json", Path(__file__).name)}
    workspace, source, manifest, snapshot = None, None, None, None
    try:
        run_bytes = read(run_path, 16 * 1024 * 1024)
        run = strict_json(run_bytes)
        manifest = run["manifest"]
        if run["schema"] != "aster.task-matrix-result.v1" or manifest["schema"] != "aster.task-matrix-run.v1":
            raise ValueError("runner report schema mismatch")
        workspace = Path(manifest["workspace"])
        source = Path(manifest["source"]["root"])
        if not workspace.is_absolute() or workspace.resolve(strict=True) != workspace or source.resolve(strict=True) != source:
            raise ValueError("canonical workspace and source required")
        report["workspace"] = str(workspace)
        if manifest["inputs"] != fixture_before:
            raise ValueError("initial input freeze differs from held-out fixture")
        oracle = strict_json(read(ROOT / "heldout-oracles.json"))[case]
        submission = oracle["submission"]
        if manifest["submissions"] != [submission] or Path(manifest["evidence_dir"]) != run_path.parent:
            raise ValueError("submission or evidence binding mismatch")
        report["run_report_sha256"] = sha(run_bytes)
        report["run_gates_passed"] = run.get("runner_gates_passed") is True
        report["immutable_inputs_before"] = check_hashes(workspace, manifest["inputs"], [submission])
        report["source_before"] = check_hashes(source, manifest["source"]["files"])
        snapshots = [item for item in run["submissions"] if item["path"] == submission and item["present"] is True]
        if len(snapshots) != 1:
            raise ValueError("one original submission snapshot required")
        snapshot = relative(run_path.parent / "submission", submission)
        submitted = read(snapshot, 16 * 1024 * 1024)
        if sha(submitted) != snapshots[0]["sha256"] or len(submitted) != snapshots[0]["bytes"] or read(relative(workspace, submission), 16 * 1024 * 1024) != submitted:
            raise ValueError("submission snapshot identity mismatch")
        report["submission_sha256"] = sha(submitted)
        if case == "analysis":
            mismatches = differences(strict_json(submitted), oracle["expected"])
            report["cases"] = [{"case": "complete_typed_json", "passed": not mismatches, "differences": mismatches, "exit_code": None}]
        else:
            report["static_imports"], report["non_stdlib_static_imports"] = static_imports(submitted)
            report["cases"] = repair_cases(submitted, read(fixture / "test_pricing.py"), oracle["independent_cases"])
        report["artifact_passed"] = all(item["passed"] for item in report["cases"]) and not report.get("non_stdlib_static_imports")
        report["submission_snapshot_unchanged"] = read(snapshot, 16 * 1024 * 1024) == submitted and read(relative(workspace, submission), 16 * 1024 * 1024) == submitted
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        report["error"] = f"{type(error).__name__}: {str(error)[:2000]}"
    finally:
        try:
            report["fixture_sources_unchanged"] = hashes(fixture) == fixture_before and all(sha(read(ROOT / name)) == digest for name, digest in protected_before.items())
            if workspace is not None and manifest is not None:
                report["immutable_inputs_after"] = check_hashes(workspace, manifest["inputs"], manifest["submissions"])
                report["source_after"] = check_hashes(source, manifest["source"]["files"])
        except (OSError, ValueError, KeyError, TypeError) as error:
            report["integrity_error"] = f"{type(error).__name__}: {str(error)[:2000]}"
        report["seconds"] = round(time.monotonic() - started, 6)
    report["passed"] = report["artifact_passed"] and report.get("run_gates_passed") is True and report.get("submission_snapshot_unchanged") is True and report.get("fixture_sources_unchanged") is True and all(report.get(key, {}).get("matched") is True for key in ("immutable_inputs_before", "immutable_inputs_after", "source_before", "source_after")) and "error" not in report and "integrity_error" not in report
    return report


def self_test():
    oracle = strict_json(read(ROOT / "heldout-oracles.json"))
    prior = strict_json(read(ROOT / "heldout-selfcheck.json"))
    assert prior["analysis_expected_matches_recomputed_data"] is True and prior["source_fixture_preserved"] is True
    expected = oracle["analysis"]["expected"]
    totals = {}
    for path in sorted((ROOT / "heldout-inputs/analysis/regions").glob("*.csv")):
        with path.open(encoding="utf-8", newline="") as source:
            for row in csv.DictReader(source):
                if row["status"] == "delivered":
                    totals[row["region"]] = totals.get(row["region"], 0) + int(row["units"])
    recomputed = {"schema_version": "regional-units-v1", "regions": [{"region": region, "delivered_units": units} for region, units in sorted(totals.items())], "total_delivered_units": sum(totals.values())}
    assert not differences(recomputed, expected)
    assert not differences(expected, expected)
    wrong = strict_json(json.dumps(expected))
    wrong["total_delivered_units"] = True
    assert differences(wrong, expected)
    for invalid in (b'{"same":1,"same":2}', b'{"bad":NaN}', b'{"bad":1e999}'):
        try:
            strict_json(invalid)
        except ValueError:
            continue
        raise AssertionError("ambiguous JSON accepted")
    original = read(ROOT / "heldout-inputs/repair/pricing.py")
    suite = read(ROOT / "heldout-inputs/repair/test_pricing.py")
    before = hashes(ROOT / "heldout-inputs")
    correct = repair_cases(b"def total_price(prices, fee):\n    return sum(prices) + fee\n", suite, oracle["repair"]["independent_cases"])
    buggy = repair_cases(original, suite, oracle["repair"]["independent_cases"])
    forged = repair_cases(b"import os\nprint('HELDOUT_RESULT={\"passed\":true}', flush=True)\nos._exit(0)\n", suite, oracle["repair"]["independent_cases"])
    assert all(item["passed"] for item in correct)
    assert buggy[0]["exit_code"] == prior["repair_original_suite_exit"] and buggy[0]["result"]["failures"] == prior["repair_original_expected_failed_tests"]
    assert sum(item["passed"] for item in buggy) == 1
    assert not any(item["passed"] for item in forged)
    with tempfile.TemporaryDirectory(prefix="heldout driver bounds ") as temporary:
        timeout = run_python(Path(temporary), "import time; time.sleep(3)", timeout=0.05)
        assert timeout["timed_out"] and timeout["direct_child_reaped"] and timeout["exit_code"] is None and timeout["supervisor_exit_code"] != 0
        excessive_output = run_python(Path(temporary), "import os; os.write(1, b'x' * (2 * 1024 * 1024)); print('HELDOUT_RESULT={\"passed\":true}')")
        assert not excessive_output["passed"] and excessive_output["stdout_bytes"] <= 1024 * 1024 and excessive_output["stdout_truncated"]
    assert hashes(ROOT / "heldout-inputs") == before
    with tempfile.TemporaryDirectory(prefix="heldout evaluator selfcheck ") as temporary:
        base = Path(temporary).resolve()
        workspace, evidence = base / "workspace", base / "evidence"
        workspace.mkdir()
        (evidence / "submission").mkdir(parents=True)
        inputs = hashes(ROOT / "heldout-inputs/analysis")
        for name in inputs:
            target = workspace / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(read(ROOT / "heldout-inputs/analysis" / name))
        submitted = json.dumps(expected).encode()
        (workspace / "report.json").write_bytes(submitted)
        (evidence / "submission/report.json").write_bytes(submitted)
        synthetic = {"schema": "aster.task-matrix-result.v1", "runner_gates_passed": True, "manifest": {"schema": "aster.task-matrix-run.v1", "workspace": str(workspace), "inputs": inputs, "source": {"root": str(ROOT), "files": {"heldout-oracles.json": sha(read(ROOT / "heldout-oracles.json"))}}, "submissions": ["report.json"], "evidence_dir": str(evidence)}, "submissions": [{"path": "report.json", "present": True, "sha256": sha(submitted), "bytes": len(submitted)}]}
        run = evidence / "run.json"
        run.write_text(json.dumps(synthetic), encoding="utf-8")
        valid = evaluate("analysis", run)
        assert valid["passed"]
        (workspace / "regions/east.csv").write_bytes(b"mutated temporary input")
        mutated = evaluate("analysis", run)
        assert not mutated["passed"] and not mutated["immutable_inputs_after"]["matched"]
    return {"schema": "aster.heldout-evaluator-selfcheck.v1", "scope": "tool self-check only; temporary correct reference and original bug, no model Run or model score", "passed": True, "analysis_recomputed_matches": True, "strict_json_types_and_keys": True, "fixture_sources_unchanged": True, "bounded_timeout": timeout, "bounded_output": excessive_output, "synthetic_report_valid": valid, "synthetic_immutable_mutation": mutated, "correct_reference": correct, "original_bug": buggy, "incomplete_forged_payload": forged}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=("analysis", "repair"))
    parser.add_argument("--run-report", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.report.exists():
        parser.error("use a new report path; previous outcomes are retained")
    if args.self_test == bool(args.case or args.run_report) or not args.self_test and not (args.case and args.run_report):
        parser.error("choose --self-test or both --case and --run-report")
    started = time.monotonic()
    try:
        report = self_test() if args.self_test else evaluate(args.case, args.run_report.resolve(strict=True))
    except Exception as error:
        report = {"passed": False, "scope": "tool self-check only" if args.self_test else "independent held-out evaluation", "error": f"{type(error).__name__}: {str(error)[:2000]}", "seconds": round(time.monotonic() - started, 6)}
    destination = args.report.resolve()
    if destination.is_relative_to(ROOT / "heldout-inputs") or "workspace" in report and destination.is_relative_to(Path(report["workspace"])):
        parser.error("independent reports must stay outside fixture and model input trees")
    report.setdefault("seconds", round(time.monotonic() - started, 6))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if len(payload.encode()) > 1024 * 1024:
        raise ValueError("bounded report exceeded")
    with args.report.open("x", encoding="utf-8") as output:
        output.write(payload)
    print(json.dumps({key: report[key] for key in ("schema", "scope", "passed", "artifact_passed", "seconds", "error") if key in report}, ensure_ascii=False))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
