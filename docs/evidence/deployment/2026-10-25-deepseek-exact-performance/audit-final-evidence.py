"""Audit retained measurements, rather than infer success from configuration."""
import gzip
import hashlib
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXPECTED = "EARLY=violet-7391-copper;MIDDLE=silver-4826-orchid;LATE=amber-9153-lantern"
FIXTURE_SHA = "539aadd60c25d9abfbfc4defab2a3c658b9abc9e78cd45355b3b0930bdd12489"


def read(relative):
    path = ROOT / relative
    assert path.stat().st_size <= 8 * 1024 * 1024, relative
    return json.loads(path.read_text(encoding="utf-8-sig"))


cohorts = []
for name, supplement in [
    ("fresh-bf16-60g-8t-baseline", None),
    ("fp4-60g-exact", "fp4-60g-8t-sse-ttft"),
    ("fp4-144g-8t-pcie75-exact", "fp4-144g-8t-sse-ttft"),
    ("fp4-144g-24t-pcie50-exact", None),
    ("fp4-144g-24t-pcie25-exact", None),
    ("fp4-144g-24t-pcie0-exact", None),
]:
    quality = read(f"{name}/quality.json")
    assert len(quality) == 5 and all(row["correct"] for row in quality), name
    decode = [row for row in read(f"{name}/decode-trials.json") if not row["warmup"]]
    assert len(decode) == (3 if name.startswith("fresh-bf16") else 6), name
    for row in decode:
        assert row["response"]["usage"]["completion_tokens"] == 128, name
        assert row["response"]["timings"]["predicted_n"] == 128, name
    prefill = read(f"{supplement or name}/prefill-trials.json")
    assert len(prefill) == 3, name
    for row in prefill:
        reply = row["response"]
        assert row["correct"] and reply["choices"][0]["finish_reason"] == "stop", name
        assert reply["timings"]["cache_n"] == 0, name
        assert reply["timings"]["prompt_n"] == reply["usage"]["prompt_tokens"], name
        assert row["ttft_seconds"] > 0 and row["sse_file"], name
    cohorts.append({
        "cohort": name,
        "quality_passed": 5,
        "warm_decode_trials": len(decode),
        "decode_tokens_per_second_median": statistics.median(
            row["response"]["timings"]["predicted_per_second"] for row in decode),
        "sse_prefill_source": supplement or name,
        "prefill_tokens_per_second_median": statistics.median(
            row["response"]["timings"]["prompt_per_second"] for row in prefill),
        "prefill_ttft_seconds_median": statistics.median(row["ttft_seconds"] for row in prefill),
    })

with gzip.open(ROOT / "fixture/prompt.txt.gz", "rb") as stream:
    fixture = stream.read(8 * 1024 * 1024 + 1)
assert len(fixture) == 5357485 and hashlib.sha256(fixture).hexdigest() == FIXTURE_SHA
native = read("native-one-million-final/result.json")
assert all(native[k] for k in ("success", "full_one_million_completed", "retrieval_passed"))
assert native["content"].strip() == EXPECTED and native["finish_reason"] == "stop"
assert native["usage"]["prompt_tokens"] == native["timings"]["prompt_n"] == 1000123
assert native["timings"]["cache_n"] == 0 and native["timings"]["predicted_n"] == 98
assert read("native-one-million-final/identity-before.json") == read("native-one-million-final/identity-after.json")
public = read("public-one-million-retrieval/result.json")
assert public["success"] and public["status"] == 200 and public["content"].strip() == EXPECTED
accounting = read("public-one-million-native-accounting.json")
assert accounting["success"] and accounting["request_delta"] == 1
assert accounting["exact_prompt_tokens"] == accounting["cached_tokens"] == 1000123
restore = read("final-selected-profile-restoration.json")
assert restore["success"] and restore["same_selected_acl"] and restore["same_engine_argv"]
assert restore["same_binary"] and restore["other_service_pids_unchanged"]
tools = read("final-workbuddy/workbuddy-tool-roundtrip.json")
assert tools["all_passed"] and len(tools["cases"]) == 6
assert all(case["passed"] and case["model"] == "deepseek-v4.1-flash" for case in tools["cases"])
http_requests = 0
for case in tools["cases"]:
    if "call_status" in case:
        assert case["call_status"] == case["result_status"] == 200
        http_requests += 2
    else:
        assert case["status"] == 200
        http_requests += 1
assert http_requests == 10
health = read("final-runtime-health.json")
assert health["all_passed"] and all(health["checks"].values())
catalog = read("final-public-model-catalog.json")
assert catalog["passed"] and catalog["status"] == 200
assert set(catalog["ids"]) == {"auto", "deepseek-v4.1-flash", "platform/apofasi", "platform/bge-m3", "platform/bge-reranker-v2-m3", "platform/pp-ocr-v6", "prism-27b", "prism-27b-alt"}
registry = read("final-power-model-registry.json")
assert registry["context_length"] == 1048576 and registry["size_bytes"] == 162954903552
assert registry["sha256"] == read("model-content-provenance.json")["expected_assembled_sha256"]
build = read("power-exact-kv-native-build-receipt.json")
assert build["binary_sha256"] == restore["power_sha256"]
assert build["source_fingerprint"] == read("source-provenance.json")["build"]["source_fingerprint"]
official = read("official-model-and-supplier-state-final.json")
assert official["model"]["state"] == "published" and official["model"]["contextWindow"] == 1048576
assert official["supplier"]["enabled"] and official["supplier"]["routable"]

audit = {
    "all_passed": True,
    "cohorts": cohorts,
    "short_decode_ratio": cohorts[-1]["decode_tokens_per_second_median"] / cohorts[0]["decode_tokens_per_second_median"],
    "native_cold_one_million_passed": True,
    "public_warm_one_million_passed": True,
    "final_selected_profile_restored": True,
    "final_workbuddy_cases": len(tools["cases"]),
    "final_workbuddy_http_requests": http_requests,
    "official_healthy": True,
    "limitations": ["Synthetic repeated archive retrieval", "Short-prompt speed ratio does not measure the 1M speed ratio", "No global quality equivalence or hardware-limit claim"],
}
(ROOT / "completion-audit.json").write_text(json.dumps(audit, indent=2) + "\n", encoding="utf-8")
print(json.dumps(audit, indent=2))
