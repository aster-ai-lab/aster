"""Run an isolated T01 submission against public data and eight independent variants."""

import argparse
import ast
import copy
import csv
import hashlib
import json
import pkgutil
import shutil
import subprocess
import sys
import sysconfig
import tempfile
import time
from pathlib import Path

from reference import MARKETS, differences, expected, records, self_test


def write_csv(path, fields, rows):
    with path.open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def mutate(input_dir, case):
    policy_path = input_dir / "policy.json"
    policy = json.loads(policy_path.read_text(encoding="utf-8"))
    if case == "zero_budget":
        policy["budget_units"] = 0
    elif case == "changed_policy":
        policy.update(budget_units=31, shipping_priorities=["High", "Critical"], shipping_saving_bp=3333,
                      discount_threshold_ppm=200001, discount_recovery_bp=1250)
        policy["shipping_cost_by_market"] = dict(zip(MARKETS, (7, 2, 5, 3, 4)))
        policy["discount_cost_by_market"] = dict(zip(MARKETS, (3, 4, 1, 6, 2)))
    elif case == "changed_data_and_references":
        path = input_dir / "Africa_orders.csv"
        orders = records(path)
        fields = list(orders[0])
        orders[0].update(Sales="1.237", Profit="-0.005", Discount="0.200001", Quantity="11")
        orders.append({**orders[0], "Order ID": orders[1]["Order ID"], "Sales": "0.005"})
        write_csv(path, fields, orders)
        # A blank CSV line must not consume a source record number.
        content = path.read_bytes()
        path.write_bytes(content.replace(b"\r\n", b"\r\n\r\n", 1))
        for filename, group in (("segment_performance.csv", "Segment"), ("shipping_by_mode.csv", "Ship_Mode")):
            path = input_dir / filename
            table = records(path)
            table[0]["Order_Count"] = "999999"
            table.append({field: "Missing Group" if field == group else "0" for field in table[0]})
            write_csv(path, list(table[0]), table)
        (input_dir / "extra provenance.txt").write_bytes(b"raw bytes\r\n\xff\x00\n")
    elif case == "negative_zero":
        order = records(input_dir / "Africa_orders.csv")[0]
        fields = list(order)
        order.update(Segment="Consumer", Sales="1.000", Profit="-0.001",
                     **{"Shipping Cost": "0", "Discount": "0", "Quantity": "1"})
        for market in MARKETS:
            write_csv(input_dir / f"{market}_orders.csv", fields,
                      [order] if market == "Africa" else [])
        (input_dir / "adjustments.jsonl").write_text("", encoding="utf-8")
        references = records(input_dir / "segment_performance.csv")
        reference = {field: "0" for field in references[0]}
        reference.update(Segment="Consumer", Order_Count="1", Total_Sales="1.00",
                         Total_Profit="-0.00", Profit_Margin="-0.10", Avg_Discount="0.0000")
        write_csv(input_dir / "segment_performance.csv", list(reference), [reference])
    elif case.startswith("synthetic_"):
        template = records(input_dir / "Africa_orders.csv")[0]
        fields = list(template)
        base = {**template, "Order ID": "duplicate-order", "Order Date": "2025-01-01", "Ship Date": "2025-01-03",
                "Segment": "Synthetic Segment", "Ship Mode": "First Class", "Order Priority": "Medium",
                "Sales": "10.000", "Profit": "0.000", "Shipping Cost": "10.000", "Discount": "0.500000", "Quantity": "2"}
        orders = {market: [] for market in MARKETS}
        orders["Africa"] = [copy.deepcopy(base), copy.deepcopy(base),
                            {**base, "Sales": "0.005", "Profit": "-0.005", "Shipping Cost": "0.005", "Discount": "0.200001"}]
        orders["Europe"] = [{**base, "Segment": "Cancelled Segment", "Order Date": "2025-02-01", "Ship Date": "2025-02-01"}]
        orders["USCA"] = [{**base, "Sales": "0", "Profit": "0", "Shipping Cost": "0", "Discount": "0.2", "Segment": "Zero Segment"}]
        policy.update(budget_units=2, shipping_saving_bp=5000, discount_recovery_bp=5000,
                      discount_threshold_ppm=200000, shipping_priorities=["Medium"],
                      shipping_cost_by_market={market: 1 for market in MARKETS},
                      discount_cost_by_market={market: 1 for market in MARKETS})
        row = "Africa_orders.csv:000003"

        def event(identifier, second, kind, amount, key=row):
            return {"event_id": identifier, "row_key": key, "timestamp": f"2025-01-01T00:00:{second:02}Z", "type": kind, "amount_milli": amount}

        events = [event("cancel", 9, "cancel", 0), event("refund1", 3, "refund", 3),
                  event("credit1", 2, "shipping_credit", 2), event("refund-over-current", 4, "refund", 3),
                  event("bool-is-not-int", 1, "cancel", False), event("credit-over-current", 5, "shipping_credit", 4),
                  event("unknown-priority", 10, "not-a-type", False), event("cancelled-priority", 11, "refund", True),
                  event("unknown-row-priority", 0, "not-a-type", False, "missing-row"),
                  event("first-payload-wins", 8, "refund", 1), event("first-payload-wins", 0, "cancel", 0),
                  event("cancel-group", 0, "cancel", 0, "Europe_orders.csv:000001"),
                  event("float-is-not-int", 0, "refund", 1.0, "USCA_orders.csv:000001")]
        if case == "synthetic_lex_tie_budget_one":
            policy["budget_units"] = 1
        elif case == "synthetic_cost_tie":
            policy["budget_units"] = 3
            policy["discount_cost_by_market"] = {market: 2 for market in MARKETS}
        elif case == "synthetic_non_greedy":
            orders = {market: [] for market in MARKETS}
            orders["Africa"] = [{**base, "Shipping Cost": "10.000"}]
            orders["Europe"] = [{**base, "Shipping Cost": "6.000"}]
            orders["LATAM"] = [{**base, "Shipping Cost": "6.000"}]
            events = []
            policy.update(budget_units=4, discount_threshold_ppm=1000000)
            policy["shipping_cost_by_market"]["Africa"] = 3
            policy["shipping_cost_by_market"]["Europe"] = 2
            policy["shipping_cost_by_market"]["LATAM"] = 2
        for market, values in orders.items():
            write_csv(input_dir / f"{market}_orders.csv", fields, values)
        (input_dir / "adjustments.jsonl").write_text("\n\n".join(json.dumps(item) for item in events) + "\n", encoding="utf-8")
    write_json(policy_path, policy)


def hashes(path):
    return {str(file.relative_to(path)): hashlib.sha256(file.read_bytes()).hexdigest() for file in sorted(path.rglob("*")) if file.is_file()}


def load_strict_json(path):
    def object_without_duplicate_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    def reject_constant(value):
        raise ValueError(f"non-finite JSON value: {value}")

    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=object_without_duplicate_keys, parse_constant=reject_constant)


def run_case(pipeline, original_input, root, case):
    case_dir = root / case
    input_dir = case_dir / "input with spaces"
    output_dir = case_dir / "output with spaces"
    unrelated_cwd = case_dir / "unrelated working directory"
    shutil.copytree(original_input, input_dir)
    unrelated_cwd.mkdir()
    if case != "public":
        mutate(input_dir, case)
    wanted = expected(input_dir)
    if case == "negative_zero":
        profit = next(item for item in wanted["reconciliation"]
                      if item["source_file"] == "segment_performance.csv"
                      and item["group"] == "Consumer" and item["field"] == "Total_Profit")
        assert profit["actual"] == profit["reference"] == "-0.00" and profit["match"]
    if case.startswith("synthetic_"):
        optimization = wanted["optimization"]
        if case == "synthetic_non_greedy":
            assert optimization["gain_milli"] == 6000 and optimization["spent_units"] == 4
            assert optimization["selected_action_ids"] == ["Europe_orders.csv:000001|shipping_switch", "LATAM_orders.csv:000001|shipping_switch"]
        else:
            assert wanted["summary"]["event_status_counts"] == {
                "applied": 5, "duplicate_event": 1, "invalid_amount": 4, "order_cancelled": 1, "unknown_row": 1, "unknown_type": 1}
            kind = "shipping_switch" if case == "synthetic_cost_tie" else "discount_review"
            count = 1 if case == "synthetic_lex_tie_budget_one" else 2
            assert optimization["gain_milli"] == 5000 * count and optimization["spent_units"] == count
            assert optimization["selected_action_ids"] == [f"Africa_orders.csv:{index:06}|{kind}" for index in range(1, count + 1)]
    before = hashes(input_dir)
    result = {"case": case, "passed": False, "input_rows": len(wanted["normalized_rows"]),
              "expected_gain_milli": wanted["optimization"]["gain_milli"], "runs": []}
    initial_bytes = None
    for index in range(2):
        started = time.monotonic()
        command = [sys.executable, "-I", str(pipeline), "--input", str(input_dir), "--output", str(output_dir)]
        try:
            process = subprocess.run(command, cwd=unrelated_cwd, capture_output=True, text=True, timeout=90)
        except subprocess.TimeoutExpired:
            result["failure"] = "timeout after 90 seconds"
            return result
        result["runs"].append({"exit_code": process.returncode, "seconds": round(time.monotonic() - started, 3),
                               "stdout": process.stdout[-2000:], "stderr": process.stderr[-2000:]})
        if process.returncode:
            result["failure"] = "nonzero exit"
            return result
        path = output_dir / "results.json"
        if not path.is_file():
            result["failure"] = "missing output/results.json"
            return result
        try:
            actual = load_strict_json(path)
        except (UnicodeDecodeError, ValueError) as error:
            result["failure"] = str(error)
            return result
        mismatches = differences(actual, wanted)
        if mismatches:
            result["differences"] = mismatches
            return result
        payload = path.read_bytes()
        if index == 0:
            initial_bytes = payload
        elif initial_bytes != payload:
            result["failure"] = "results.json is not byte-identical on repeated execution"
            return result
        if hashes(input_dir) != before:
            result["failure"] = "input files changed"
            return result
    result.update(passed=True, deterministic=True, input_unchanged=True,
                  results_sha256=hashlib.sha256(initial_bytes).hexdigest())
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pipeline", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    self_test()
    pipeline, original_input = args.pipeline.resolve(), args.input.resolve()
    before = hashes(original_input)
    tree = ast.parse(pipeline.read_text(encoding="utf-8"))
    imports = sorted({name.name.split(".")[0] for node in ast.walk(tree) if isinstance(node, ast.Import) for name in node.names}
                     | {node.module.split(".")[0] for node in ast.walk(tree) if isinstance(node, ast.ImportFrom) and node.module})
    stdlib = set(getattr(sys, "stdlib_module_names", sys.builtin_module_names))
    if not hasattr(sys, "stdlib_module_names"):
        standard_path = Path(sysconfig.get_path("stdlib"))
        stdlib.update(module.name for module in pkgutil.iter_modules([str(standard_path), str(standard_path / "lib-dynload")]))
    external = sorted(set(imports) - stdlib)
    cases = ("public", "zero_budget", "changed_policy", "changed_data_and_references", "synthetic_row_exclusivity",
             "synthetic_lex_tie_budget_one", "synthetic_cost_tie", "synthetic_non_greedy", "negative_zero")
    with tempfile.TemporaryDirectory(prefix="t01 independent eval ") as temp:
        root = Path(temp)
        isolated_pipeline = root / "pipeline.py"
        shutil.copyfile(pipeline, isolated_pipeline)
        results = [run_case(isolated_pipeline, original_input, root, case) for case in cases]
    report = {"pipeline_sha256": hashlib.sha256(pipeline.read_bytes()).hexdigest(),
              "oracle_self_test": "270 DP versus brute-force cases passed", "static_imports": imports,
              "non_stdlib_static_imports": external, "original_input_unchanged": before == hashes(original_input),
              "passed_cases": sum(item["passed"] for item in results), "total_cases": len(results), "cases": results}
    report["passed"] = all(item["passed"] for item in results) and not external and report["original_input_unchanged"]
    args.report.parent.mkdir(parents=True, exist_ok=True)
    write_json(args.report, report)
    print(json.dumps({key: value for key, value in report.items() if key != "cases"}, ensure_ascii=False, indent=2))
    for case in results:
        print(f"{case['case']}: {'PASS' if case['passed'] else 'FAIL'}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
