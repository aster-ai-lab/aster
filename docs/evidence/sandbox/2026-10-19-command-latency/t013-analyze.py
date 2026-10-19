"""Recompute the fixed T013 comparison from retained raw round logs."""

import json
import math
import statistics
from pathlib import Path

EVIDENCE = Path(__file__).resolve().parent
PHASES = (
    "host_connect", "bundle_prepare", "vm_create", "process_start",
    "collect_wait", "vm_delete", "host_shutdown", "scratch_remove",
)


def distribution(values):
    values = sorted(values)
    return {
        "count": len(values),
        "meanMs": statistics.fmean(values),
        "p50Ms": statistics.median(values),
        "p95Ms": values[math.ceil(len(values) * 0.95) - 1],
        "minMs": values[0],
        "maxMs": values[-1],
        "populationStdDevMs": statistics.pstdev(values),
    }


def read_round(record, failures):
    name = record["name"]
    samples = []
    current = None
    for number, line in enumerate((EVIDENCE / f"t013-round-{name}.log").read_text().splitlines(), 1):
        if "OCI_TIMING_BEGIN " in line:
            begin = json.loads(line.partition("OCI_TIMING_BEGIN ")[2])
            if current is not None:
                failures.append({"round": name, "line": number, "reason": "missing command receipt"})
            current = {"sample": begin["sample"], "timeoutMs": begin["timeout_ms"], "phasesMs": {}}
        elif line.startswith("{"):
            fields = json.loads(line).get("fields", {})
            if "phase" in fields:
                phase, elapsed = fields["phase"], fields["elapsed_ms"]
                if current is None or phase not in PHASES or phase in current["phasesMs"]:
                    failures.append({"round": name, "line": number, "reason": "unbound, unknown or duplicate phase"})
                elif not math.isfinite(elapsed) or elapsed < 0:
                    failures.append({"round": name, "line": number, "reason": "invalid phase duration"})
                else:
                    current["phasesMs"][phase] = elapsed
        elif line.startswith("OCI_TIMING "):
            receipt = json.loads(line.removeprefix("OCI_TIMING "))
            if current is None or current["sample"] != receipt["sample"]:
                failures.append({"round": name, "line": number, "reason": "unbound command receipt"})
                current = {"phasesMs": {}}
            current.update(receipt)
            valid = (
                receipt["error"] is None and receipt["exit_code"] == 0
                and receipt["stdout"] == "phase-probe" and receipt["stderr"] == ""
                and receipt["timed_out"] is False
                and set(current["phasesMs"]) == set(PHASES)
                and math.isfinite(receipt["total_ms"]) and receipt["total_ms"] > 0
                and sum(current["phasesMs"].values()) <= receipt["total_ms"] + 0.001
            )
            if not valid:
                failures.append({"round": name, "sample": receipt["sample"], "reason": "command receipt or phase contract failed"})
            current["valid"] = valid
            current["unattributedMs"] = receipt["total_ms"] - sum(current["phasesMs"].values())
            samples.append(current)
            current = None
    if current is not None:
        failures.append({"round": name, "sample": current["sample"], "reason": "round ended without a command receipt"})
    if record["exitCode"] != 0 or [s["sample"] for s in samples] != list(range(19)):
        failures.append({"round": name, "reason": "round did not pass with exactly 19 ordered samples"})
    return {**record, "samples": samples}


def main():
    failures = []
    execution = json.loads((EVIDENCE / "t013-round-execution.json").read_text())
    controls = json.loads((EVIDENCE / "t013-build-controls.json").read_text())
    rounds = [read_round(record, failures) for record in execution["rounds"]]
    if [r["name"] for r in rounds] != controls["roundOrder"]:
        failures.append({"reason": "the four predeclared balanced rounds are incomplete"})
    result = {
        "schema": "aster.sandbox.t013.comparison.v1",
        "qualified": not failures,
        "scope": "Same-source software versus hardware SHA features; isolated OCI printf calls only.",
        "method": "p50 is the median; p95 uses nearest rank. Small serial samples do not establish reliable tail latency.",
        "order": controls["roundOrder"],
        "rounds": rounds,
        "failures": failures,
        "explicitlyNotClaimed": [
            "No end-to-end T01, model latency or product deployment improvement.",
            "No filesystem-cache flush or cold-machine result; first means first in that round.",
            "No Windows/Linux qualification or VM reuse.",
        ],
    }
    if not failures:
        variants = {}
        for variant in ("software", "hardware"):
            owned = [r for r in rounds if r["variant"] == variant]
            samples = [s for r in owned for s in r["samples"]]
            variants[variant] = {
                "allSamples": distribution([s["total_ms"] for s in samples]),
                "subsequentSamples": distribution([s["total_ms"] for s in samples if s["sample"] > 0]),
                "firstSamplesMs": [r["samples"][0]["total_ms"] for r in owned],
                "roundDistributions": {r["name"]: distribution([s["total_ms"] for s in r["samples"]]) for r in owned},
                "phases": {phase: distribution([s["phasesMs"][phase] for s in samples]) for phase in PHASES},
            }
        result["variants"] = variants
        software = variants["software"]["allSamples"]["meanMs"]
        hardware = variants["hardware"]["allSamples"]["meanMs"]
        result["meanReductionPercent"] = (software - hardware) / software * 100
    (EVIDENCE / "t013-comparison.json").write_text(json.dumps(result, indent=2) + "\n")
    assert not failures, failures
    print(json.dumps({"qualified": True, "samples": 76, "variants": result["variants"]}, indent=2))


if __name__ == "__main__":
    main()
