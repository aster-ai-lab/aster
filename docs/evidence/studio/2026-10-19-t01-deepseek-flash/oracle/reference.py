"""Independent T01 oracle. Keep this file and its outputs away from the agent under test."""

import argparse
import csv
import hashlib
import itertools
import json
from collections import Counter, defaultdict
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, getcontext
from pathlib import Path

getcontext().prec = 60
MARKETS = ("Africa", "Asia_Pacific", "Europe", "LATAM", "USCA")


def records(path):
    with path.open(encoding="utf-8-sig", newline="") as source:
        return list(csv.DictReader(source))


def scaled(value, scale):
    exact = Decimal(value) * scale
    assert exact == exact.to_integral_value(), (value, scale)
    return int(exact)


def rounded(value, places=0):
    return format(Decimal(value).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP), f".{places}f")


def ratio(numerator, denominator):
    return int(rounded(Decimal(numerator) * 10000 / denominator)) if denominator else None


def metrics(rows):
    active = [row for row in rows if row.get("active", True)]
    sales = sum(row["sales_milli"] for row in active)
    profit = sum(row["profit_milli"] for row in active)
    shipping = sum(row["shipping_milli"] for row in active)
    return {
        "row_count": len(active),
        "quantity": sum(row["quantity"] for row in active),
        "sales_milli": sales,
        "profit_milli": profit,
        "shipping_milli": shipping,
        "contribution_milli": profit - shipping,
        "profit_margin_bp": ratio(profit, sales),
        "contribution_margin_bp": ratio(profit - shipping, sales),
        "high_discount_count": sum(row["discount_ppm"] > 200000 for row in active),
        "negative_contribution_count": sum(row["profit_milli"] < row["shipping_milli"] for row in active),
    }


def choose(candidates, budget):
    """Multiple-choice exact-cost DP, retaining the lexicographically first tied solution."""
    states = {0: (0, ())}
    for _, group in itertools.groupby(candidates, key=lambda item: item["row_key"]):
        options = list(group)
        next_states = dict(states)
        for spent, (gain, ids) in states.items():
            for option in options:
                cost = spent + option["cost_units"]
                if cost > budget:
                    continue
                proposal = gain + option["gain_milli"], ids + (option["action_id"],)
                current = next_states.get(cost)
                if current is None or proposal[0] > current[0] or (proposal[0] == current[0] and proposal[1] < current[1]):
                    next_states[cost] = proposal
        states = next_states
    cost, (gain, ids) = min(states.items(), key=lambda item: (-item[1][0], item[0], item[1][1]))
    return cost, gain, list(ids)


def expected(input_dir):
    input_dir = Path(input_dir)
    products = {row["Product ID"]: row for row in records(input_dir / "product_info.csv")}
    normalized = []
    for market in MARKETS:
        name = f"{market}_orders.csv"
        for index, row in enumerate(records(input_dir / name), 1):
            product = products[row["Product ID"]]
            normalized.append({
                "row_key": f"{name}:{index:06}", "source_file": name, "source_row": index,
                "order_id": row["Order ID"], "product_id": row["Product ID"], "market": market,
                "category": product["Category"], "sub_category": product["Sub-Category"],
                "segment": row["Segment"], "ship_mode": row["Ship Mode"], "priority": row["Order Priority"],
                "order_date": row["Order Date"], "ship_date": row["Ship Date"],
                "delivery_days": (date.fromisoformat(row["Ship Date"]) - date.fromisoformat(row["Order Date"])).days,
                "quantity": int(row["Quantity"]), "sales_milli": scaled(row["Sales"], 1000),
                "profit_milli": scaled(row["Profit"], 1000), "shipping_milli": scaled(row["Shipping Cost"], 1000),
                "discount_ppm": scaled(row["Discount"], 1000000),
            })
    normalized.sort(key=lambda row: row["row_key"])
    effective = [{**row, "active": True} for row in normalized]
    by_key = {row["row_key"]: row for row in effective}
    events = [json.loads(line) for line in (input_dir / "adjustments.jsonl").read_text(encoding="utf-8").splitlines() if line.strip()]
    audit = [{"input_line": index, "event_id": event["event_id"], "row_key": event["row_key"], "status": "duplicate_event"}
             for index, event in enumerate(events, 1)]
    first_events = []
    seen = set()
    for index, event in enumerate(events):
        if event["event_id"] not in seen:
            first_events.append((index, event))
            seen.add(event["event_id"])
    for index, event in sorted(first_events, key=lambda item: (item[1]["timestamp"], item[1]["event_id"])):
        row, kind, amount = by_key.get(event["row_key"]), event["type"], event["amount_milli"]
        if row is None:
            status = "unknown_row"
        elif kind not in ("cancel", "refund", "shipping_credit"):
            status = "unknown_type"
        elif not row["active"]:
            status = "order_cancelled"
        elif type(amount) is not int or (kind == "cancel" and amount != 0) or (kind == "refund" and not 0 < amount <= row["sales_milli"]) or (kind == "shipping_credit" and not 0 < amount <= row["shipping_milli"]):
            status = "invalid_amount"
        else:
            status = "applied"
            if kind == "cancel":
                row.update(active=False, sales_milli=0, profit_milli=0, shipping_milli=0)
            elif kind == "refund":
                row["sales_milli"] -= amount
                row["profit_milli"] -= amount
            else:
                row["shipping_milli"] -= amount
        audit[index]["status"] = status

    aggregates = {}
    for name, fields in (("market", ("market",)), ("market_category", ("market", "category")),
                         ("segment", ("segment",)), ("ship_mode", ("ship_mode",)),
                         ("market_month", ("market", "month"))):
        groups = defaultdict(list)
        for row in effective:
            key = tuple(row["order_date"][:7] if field == "month" else row[field] for field in fields)
            groups[key].append(row)
        aggregates[name] = [{**dict(zip(fields, key)), **metrics(groups[key])} for key in sorted(groups)]
    summary = metrics(effective)
    summary.update(input_row_count=len(normalized), unique_order_id_count=len({row["order_id"] for row in normalized}),
                   duplicate_order_id_extra_rows=len(normalized) - len({row["order_id"] for row in normalized}),
                   cancelled_row_count=sum(not row["active"] for row in effective),
                   event_status_counts=dict(sorted(Counter(item["status"] for item in audit).items())))

    reconciliation = []
    for name, source_field, key in (("segment_performance.csv", "Segment", "segment"), ("shipping_by_mode.csv", "Ship_Mode", "ship_mode")):
        for reference in records(input_dir / name):
            group = reference[source_field]
            rows = [row for row in normalized if row[key] == group]
            count = len(rows)
            sales = sum(row["sales_milli"] for row in rows)
            profit = sum(row["profit_milli"] for row in rows)
            shipping = sum(row["shipping_milli"] for row in rows)
            if key == "segment":
                values = {"Order_Count": (count, 0), "Total_Sales": (Decimal(sales) / 1000, 2),
                          "Total_Profit": (Decimal(profit) / 1000, 2),
                          "Profit_Margin": (Decimal(profit) * 100 / sales if sales else 0, 2),
                          "Avg_Discount": (Decimal(sum(row["discount_ppm"] for row in rows)) / 1000000 / count if count else 0, 4)}
            else:
                values = {"Order_Count": (count, 0), "Total_Shipping_Cost": (Decimal(shipping) / 1000, 2),
                          "Avg_Shipping_Cost": (Decimal(shipping) / 1000 / count if count else 0, 2),
                          "Avg_Delivery_Days": (Decimal(sum(row["delivery_days"] for row in rows)) / count if count else 0, 2)}
            for field, (value, places) in values.items():
                actual, expected_reference = rounded(value, places), rounded(reference[field], places)
                reconciliation.append({"source_file": name, "group": group, "field": field, "actual": actual,
                                       "reference": expected_reference, "match": actual == expected_reference})
    reconciliation.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))

    policy = json.loads((input_dir / "policy.json").read_text(encoding="utf-8"))
    candidates = []
    for row in effective:
        if not row["active"]:
            continue
        options = []
        if row["ship_mode"] != "Standard Class" and row["priority"] in policy["shipping_priorities"]:
            options.append(("shipping_switch", row["shipping_milli"] * policy["shipping_saving_bp"] // 10000, policy["shipping_cost_by_market"][row["market"]]))
        if row["discount_ppm"] > policy["discount_threshold_ppm"] and row["profit_milli"] < row["shipping_milli"]:
            options.append(("discount_review", row["sales_milli"] * policy["discount_recovery_bp"] // 10000, policy["discount_cost_by_market"][row["market"]]))
        for kind, gain, cost in options:
            if gain > 0:
                candidates.append({"action_id": row["row_key"] + "|" + kind, "row_key": row["row_key"], "type": kind,
                                   "cost_units": cost, "gain_milli": gain})
    candidates.sort(key=lambda item: item["action_id"])
    spent, gain, ids = choose(candidates, policy["budget_units"])
    chosen = set(ids)
    return {"schema_version": "107-v1", "normalized_rows": normalized, "event_audit": audit, "effective_rows": effective,
            "aggregates": aggregates, "reconciliation": reconciliation, "summary": summary, "action_candidates": candidates,
            "selected_actions": [item for item in candidates if item["action_id"] in chosen],
            "optimization": {"budget_units": policy["budget_units"], "spent_units": spent, "gain_milli": gain,
                             "selected_action_ids": ids, "projected_contribution_milli": summary["contribution_milli"] + gain},
            "input_sha256": {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                             for path in sorted(input_dir.iterdir()) if path.is_file()}}


def differences(actual, wanted, path="$", limit=20):
    differences_found = []

    def visit(left, right, position):
        if len(differences_found) >= limit:
            return
        if type(left) is not type(right):
            differences_found.append(f"{position}: type {type(left).__name__}, expected {type(right).__name__}")
        elif isinstance(right, dict):
            if left.keys() != right.keys():
                differences_found.append(f"{position}: missing={sorted(right.keys() - left.keys())}, extra={sorted(left.keys() - right.keys())}")
            for key in right.keys() & left.keys():
                visit(left[key], right[key], f"{position}.{key}")
        elif isinstance(right, list):
            if len(left) != len(right):
                differences_found.append(f"{position}: length {len(left)}, expected {len(right)}")
            for index, (a, b) in enumerate(zip(left, right)):
                visit(a, b, f"{position}[{index}]")
        elif left != right:
            differences_found.append(f"{position}: {left!r}, expected {right!r}")

    visit(actual, wanted, path)
    return differences_found


def self_test():
    assert scaled("1.237", 1000) == 1237
    assert scaled("0.200001", 1000000) == 200001
    assert rounded("-1.005", 2) == "-1.01"
    assert ratio(-1, 20000) == -1 and ratio(1, 0) is None
    assert differences({"x": True}, {"x": 1})
    # Exhaustive enumeration is an independent check of DP objectives and row exclusivity.
    for seed in range(30):
        options = []
        for row in range(4):
            for kind in ("a", "b"):
                options.append({"action_id": f"{row}|{kind}", "row_key": str(row),
                                "cost_units": 1 + (seed + row + ord(kind)) % 4,
                                "gain_milli": 1 + (seed * 3 + row * 7 + ord(kind)) % 9})
        for budget in range(9):
            possible = []
            for subset in itertools.product((None, 0, 1), repeat=4):
                selected = [options[row * 2 + kind] for row, kind in enumerate(subset) if kind is not None]
                cost = sum(item["cost_units"] for item in selected)
                if cost <= budget:
                    possible.append((cost, sum(item["gain_milli"] for item in selected), [item["action_id"] for item in selected]))
            assert choose(options, budget) == min(possible, key=lambda item: (-item[1], item[0], item[2]))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--compare", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("oracle self-test: passed (270 DP versus brute-force cases)")
    if args.input:
        result = expected(args.input)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if args.compare:
            mismatches = differences(json.loads(args.compare.read_text(encoding="utf-8")), result)
            print(json.dumps({"match": not mismatches, "differences": mismatches}, ensure_ascii=False, indent=2))
            raise SystemExit(bool(mismatches))
        print(json.dumps({"summary": result["summary"], "optimization": result["optimization"],
                          "candidate_count": len(result["action_candidates"]),
                          "reconciliation_mismatches": [item for item in result["reconciliation"] if not item["match"]]}, ensure_ascii=False, indent=2))
