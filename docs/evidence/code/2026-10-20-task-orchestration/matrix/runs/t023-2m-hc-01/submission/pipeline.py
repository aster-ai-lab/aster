#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""Global order audit, event processing and budget optimization (schema 107-v1).

Reads an input directory containing five *_orders.csv files, product_info.csv,
segment_performance.csv, shipping_by_mode.csv, priority_codebook.txt,
policy.json and adjustments.jsonl, and writes output/results.json.
"""

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP

SCHEMA_VERSION = "107-v1"
ORDERS_SUFFIX = "_orders.csv"


def scaled_int(text, scale):
    """Convert a decimal string to an exact integer scaled by ``scale``."""
    if text is None:
        return 0
    s = str(text).strip()
    if not s:
        return 0
    value = Decimal(s) * scale
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def round_half_up_ratio(num, den, mul=1):
    """Exact ROUND_HALF_UP of num * mul / den (0 when den == 0)."""
    if den == 0:
        return 0
    n = num * mul
    negative = n < 0
    n = -n if negative else n
    d = -den if den < 0 else den
    q, r = divmod(n, d)
    if 2 * r >= d:
        q += 1
    return -q if negative else q


def round_half_up_ratio_or_none(num, den, mul=1):
    if den == 0:
        return None
    return round_half_up_ratio(num, den, mul)


def format_fixed(value, decimals):
    """Format an integer scaled by 10**decimals as a fixed-point string."""
    negative = value < 0
    v = -value if negative else value
    scale = 10 ** decimals
    whole, frac = divmod(v, scale)
    if decimals:
        body = "%d.%0*d" % (whole, decimals, frac)
    else:
        body = str(whole)
    return "-" + body if (negative and v != 0) else body


def format_reference(text, decimals):
    """Normalise a reference table cell to a fixed-point string."""
    s = "" if text is None else str(text).strip()
    if s == "":
        dec = Decimal(0)
    else:
        try:
            dec = Decimal(s)
        except Exception:
            dec = Decimal(0)
    quantum = Decimal(1).scaleb(-decimals)
    q = dec.quantize(quantum, rounding=ROUND_HALF_UP)
    if q == 0:
        q = abs(q)
    return ("{:." + str(decimals) + "f}").format(q)


def read_csv_rows(path):
    if not os.path.isfile(path):
        return []
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        rows = []
        for row in reader:
            if row is None:
                continue
            rows.append(row)
        return rows


def cell(row, name):
    value = row.get(name)
    if value is None:
        return ""
    return str(value).strip()


def load_products(input_dir):
    products = {}
    for row in read_csv_rows(os.path.join(input_dir, "product_info.csv")):
        pid = cell(row, "Product ID")
        if pid and pid not in products:
            products[pid] = (cell(row, "Category"), cell(row, "Sub-Category"))
    return products


def load_orders(input_dir):
    products = load_products(input_dir)
    names = sorted(
        name
        for name in os.listdir(input_dir)
        if name.endswith(ORDERS_SUFFIX) and os.path.isfile(os.path.join(input_dir, name))
    )
    rows = []
    for name in names:
        market = name[: -len(ORDERS_SUFFIX)]
        records = read_csv_rows(os.path.join(input_dir, name))
        for index, raw in enumerate(records, start=1):
            pid = cell(raw, "Product ID")
            category, sub_category = products.get(pid, ("", ""))
            order_date = cell(raw, "Order Date")
            ship_date = cell(raw, "Ship Date")
            try:
                delivery_days = (date.fromisoformat(ship_date) - date.fromisoformat(order_date)).days
            except Exception:
                delivery_days = 0
            rows.append({
                "row_key": "%s:%06d" % (name, index),
                "source_file": name,
                "source_row": index,
                "order_id": cell(raw, "Order ID"),
                "product_id": pid,
                "market": market,
                "category": category,
                "sub_category": sub_category,
                "segment": cell(raw, "Segment"),
                "ship_mode": cell(raw, "Ship Mode"),
                "priority": cell(raw, "Order Priority"),
                "order_date": order_date,
                "ship_date": ship_date,
                "delivery_days": delivery_days,
                "quantity": scaled_int(cell(raw, "Quantity"), 1),
                "sales_milli": scaled_int(cell(raw, "Sales"), 1000),
                "profit_milli": scaled_int(cell(raw, "Profit"), 1000),
                "shipping_milli": scaled_int(cell(raw, "Shipping Cost"), 1000),
                "discount_ppm": scaled_int(cell(raw, "Discount"), 1000000),
            })
    rows.sort(key=lambda item: item["row_key"])
    return rows


def load_policy(input_dir):
    path = os.path.join(input_dir, "policy.json")
    if not os.path.isfile(path):
        return {}
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def load_events(input_dir):
    path = os.path.join(input_dir, "adjustments.jsonl")
    records = []
    if not os.path.isfile(path):
        return records
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            records.append(json.loads(line))
    return records


def is_integer_amount(value):
    return isinstance(value, int) and not isinstance(value, bool)


def process_events(normalized, raw_events):
    states = {}
    for row in normalized:
        states[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    seen = set()
    entries = []
    for position, obj in enumerate(raw_events):
        event_id = obj.get("event_id")
        duplicate = event_id in seen
        if not duplicate:
            seen.add(event_id)
        entries.append({
            "line": position + 1,
            "obj": obj,
            "duplicate": duplicate,
            "status": "duplicate_event" if duplicate else None,
        })

    pending = [entry for entry in entries if not entry["duplicate"]]
    pending.sort(key=lambda entry: (
        str(entry["obj"].get("timestamp", "")),
        str(entry["obj"].get("event_id", "")),
    ))

    cancelled = set()
    valid_types = ("cancel", "refund", "shipping_credit")
    for entry in pending:
        obj = entry["obj"]
        row_key = obj.get("row_key")
        event_type = obj.get("type")
        amount = obj.get("amount_milli")
        status = None
        if row_key not in states:
            status = "unknown_row"
        elif event_type not in valid_types:
            status = "unknown_type"
        elif row_key in cancelled:
            status = "order_cancelled"
        elif not is_integer_amount(amount):
            status = "invalid_amount"
        elif event_type == "cancel":
            if amount != 0:
                status = "invalid_amount"
        elif event_type == "refund":
            if amount <= 0 or amount > states[row_key]["sales_milli"]:
                status = "invalid_amount"
        else:
            if amount <= 0 or amount > states[row_key]["shipping_milli"]:
                status = "invalid_amount"

        if status is None:
            status = "applied"
            state = states[row_key]
            if event_type == "cancel":
                state["active"] = False
                state["sales_milli"] = 0
                state["profit_milli"] = 0
                state["shipping_milli"] = 0
                cancelled.add(row_key)
            elif event_type == "refund":
                state["sales_milli"] -= amount
                state["profit_milli"] -= amount
            else:
                state["shipping_milli"] -= amount
        entry["status"] = status

    audit = []
    for entry in entries:
        obj = entry["obj"]
        audit.append({
            "input_line": entry["line"],
            "event_id": obj.get("event_id"),
            "row_key": obj.get("row_key"),
            "status": entry["status"],
        })
    return states, audit


def build_effective_rows(normalized, states):
    effective = []
    for row in normalized:
        state = states[row["row_key"]]
        copy = dict(row)
        copy["sales_milli"] = state["sales_milli"]
        copy["profit_milli"] = state["profit_milli"]
        copy["shipping_milli"] = state["shipping_milli"]
        copy["active"] = state["active"]
        effective.append(copy)
    return effective


def compute_metrics(rows):
    row_count = len(rows)
    quantity = sum(row["quantity"] for row in rows)
    sales = sum(row["sales_milli"] for row in rows)
    profit = sum(row["profit_milli"] for row in rows)
    shipping = sum(row["shipping_milli"] for row in rows)
    contribution = profit - shipping
    return {
        "row_count": row_count,
        "quantity": quantity,
        "sales_milli": sales,
        "profit_milli": profit,
        "shipping_milli": shipping,
        "contribution_milli": contribution,
        "profit_margin_bp": round_half_up_ratio_or_none(profit, sales, 10000),
        "contribution_margin_bp": round_half_up_ratio_or_none(contribution, sales, 10000),
        "high_discount_count": sum(1 for row in rows if row["discount_ppm"] > 200000),
        "negative_contribution_count": sum(
            1 for row in rows if row["profit_milli"] < row["shipping_milli"]
        ),
    }


GROUP_SPECS = (
    ("market", ("market",)),
    ("market_category", ("market", "category")),
    ("segment", ("segment",)),
    ("ship_mode", ("ship_mode",)),
    ("market_month", ("market", "month")),
)


def group_value(row, field):
    if field == "month":
        return row["order_date"][:7]
    return row[field]


def build_aggregates(normalized, effective):
    aggregates = {}
    for name, fields in GROUP_SPECS:
        keys = set()
        for row in normalized:
            keys.add(tuple(group_value(row, field) for field in fields))
        buckets = {key: [] for key in keys}
        for row in effective:
            if not row["active"]:
                continue
            key = tuple(group_value(row, field) for field in fields)
            if key in buckets:
                buckets[key].append(row)
        items = []
        for key in sorted(buckets):
            item = {}
            for field, value in zip(fields, key):
                item[field] = value
            item.update(compute_metrics(buckets[key]))
            items.append(item)
        aggregates[name] = items
    return aggregates


def build_summary(normalized, effective, audit):
    active_rows = [row for row in effective if row["active"]]
    summary = compute_metrics(active_rows)
    order_ids = set(row["order_id"] for row in normalized)
    summary["input_row_count"] = len(normalized)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized) - len(order_ids)
    summary["cancelled_row_count"] = sum(1 for row in effective if not row["active"])
    counts = {}
    for entry in audit:
        status = entry["status"]
        counts[status] = counts.get(status, 0) + 1
    summary["event_status_counts"] = {key: counts[key] for key in sorted(counts)}
    return summary


def group_stats(rows):
    return {
        "count": len(rows),
        "sales": sum(row["sales_milli"] for row in rows),
        "profit": sum(row["profit_milli"] for row in rows),
        "shipping": sum(row["shipping_milli"] for row in rows),
        "discount": sum(row["discount_ppm"] for row in rows),
        "days": sum(row["delivery_days"] for row in rows),
    }


def actual_text(field, stats):
    count = stats["count"]
    if field == "Order_Count":
        return format_fixed(count, 0)
    if field == "Total_Sales":
        return format_fixed(round_half_up_ratio(stats["sales"], 1000, 100), 2)
    if field == "Total_Profit":
        return format_fixed(round_half_up_ratio(stats["profit"], 1000, 100), 2)
    if field == "Profit_Margin":
        return format_fixed(round_half_up_ratio(stats["profit"], stats["sales"], 10000), 2)
    if field == "Avg_Discount":
        return format_fixed(round_half_up_ratio(stats["discount"], count * 100), 4)
    if field == "Total_Shipping_Cost":
        return format_fixed(round_half_up_ratio(stats["shipping"], 1000, 100), 2)
    if field == "Avg_Shipping_Cost":
        return format_fixed(round_half_up_ratio(stats["shipping"], count * 1000, 100), 2)
    if field == "Avg_Delivery_Days":
        return format_fixed(round_half_up_ratio(stats["days"], count, 100), 2)
    return ""


REFERENCE_TABLES = (
    (
        "segment_performance.csv",
        "Segment",
        "segment",
        (
            ("Order_Count", 0),
            ("Total_Sales", 2),
            ("Total_Profit", 2),
            ("Profit_Margin", 2),
            ("Avg_Discount", 4),
        ),
    ),
    (
        "shipping_by_mode.csv",
        "Ship_Mode",
        "ship_mode",
        (
            ("Order_Count", 0),
            ("Total_Shipping_Cost", 2),
            ("Avg_Shipping_Cost", 2),
            ("Avg_Delivery_Days", 2),
        ),
    ),
)


def build_reconciliation(input_dir, normalized):
    results = []
    for source_file, group_column, row_field, fields in REFERENCE_TABLES:
        reference_rows = read_csv_rows(os.path.join(input_dir, source_file))
        buckets = {}
        for row in normalized:
            buckets.setdefault(row[row_field], []).append(row)
        for ref_row in reference_rows:
            group = cell(ref_row, group_column)
            stats = group_stats(buckets.get(group, []))
            for field, decimals in fields:
                actual = actual_text(field, stats)
                reference = format_reference(ref_row.get(field), decimals)
                results.append({
                    "source_file": source_file,
                    "group": group,
                    "field": field,
                    "actual": actual,
                    "reference": reference,
                    "match": actual == reference,
                })
    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


def build_candidates(effective, policy):
    priorities = set(policy.get("shipping_priorities", []) or [])
    shipping_bp = int(policy.get("shipping_saving_bp", 0) or 0)
    threshold_ppm = int(policy.get("discount_threshold_ppm", 0) or 0)
    recovery_bp = int(policy.get("discount_recovery_bp", 0) or 0)
    shipping_costs = policy.get("shipping_cost_by_market", {}) or {}
    discount_costs = policy.get("discount_cost_by_market", {}) or {}
    candidates = []
    for row in effective:
        market = row["market"]
        row_key = row["row_key"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = (row["shipping_milli"] * shipping_bp) // 10000
            if gain > 0:
                cost = shipping_costs.get(market)
                if cost is not None:
                    candidates.append({
                        "action_id": row_key + "|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": int(cost),
                        "gain_milli": int(gain),
                    })
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > threshold_ppm and contribution < 0:
            gain = (row["sales_milli"] * recovery_bp) // 10000
            if gain > 0:
                cost = discount_costs.get(market)
                if cost is not None:
                    candidates.append({
                        "action_id": row_key + "|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": int(cost),
                        "gain_milli": int(gain),
                    })
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimize(candidates, budget):
    by_row = {}
    for candidate in candidates:
        by_row.setdefault(candidate["row_key"], []).append(candidate)
    row_keys = sorted(by_row.keys(), reverse=True)
    max_cost = sum(
        max(option["cost_units"] for option in by_row[row_key]) for row_key in row_keys
    )
    cap = min(budget, max_cost)
    dp = [None] * (cap + 1)
    dp[0] = (0, ())
    for row_key in row_keys:
        options = by_row[row_key]
        new_dp = list(dp)
        for cost_state in range(cap + 1):
            current = dp[cost_state]
            if current is None:
                continue
            gain_state, ids_state = current
            for option in options:
                next_cost = cost_state + option["cost_units"]
                if next_cost > cap:
                    continue
                candidate = (gain_state + option["gain_milli"], (option["action_id"],) + ids_state)
                existing = new_dp[next_cost]
                if (
                    existing is None
                    or candidate[0] > existing[0]
                    or (candidate[0] == existing[0] and candidate[1] < existing[1])
                ):
                    new_dp[next_cost] = candidate
        dp = new_dp
    best = None
    for cost_state in range(cap + 1):
        current = dp[cost_state]
        if current is None:
            continue
        gain_state, ids_state = current
        key = (-gain_state, cost_state, ids_state)
        if best is None or key < best[0]:
            best = (key, cost_state, gain_state, ids_state)
    if best is None:
        return [], 0, 0
    _, spent, gain, ids = best
    selected_ids = set(ids)
    selected = [c for c in candidates if c["action_id"] in selected_ids]
    selected.sort(key=lambda item: item["action_id"])
    return selected, spent, gain


def sha256_map(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path):
            continue
        with open(path, "rb") as handle:
            digests[name] = hashlib.sha256(handle.read()).hexdigest()
    return digests


def run(input_dir, output_dir):
    normalized = load_orders(input_dir)
    policy = load_policy(input_dir)
    raw_events = load_events(input_dir)
    states, audit = process_events(normalized, raw_events)
    effective = build_effective_rows(normalized, states)
    aggregates = build_aggregates(normalized, effective)
    reconciliation = build_reconciliation(input_dir, normalized)
    summary = build_summary(normalized, effective, audit)
    candidates = build_candidates(effective, policy)
    budget = int(policy.get("budget_units", 0) or 0)
    selected, spent, gain = optimize(candidates, budget)
    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"] for row in effective if row["active"]
    )
    results = {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized,
        "event_audit": audit,
        "effective_rows": effective,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected,
        "optimization": {
            "budget_units": budget,
            "spent_units": spent,
            "gain_milli": gain,
            "selected_action_ids": [c["action_id"] for c in selected],
            "projected_contribution_milli": current_contribution + gain,
        },
        "input_sha256": sha256_map(input_dir),
    }
    os.makedirs(output_dir, exist_ok=True)
    target = os.path.join(output_dir, "results.json")
    payload = json.dumps(results, ensure_ascii=False, sort_keys=True, indent=2)
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(payload)
        handle.write("\n")
    return target


def main():
    parser = argparse.ArgumentParser(description="107-v1 order audit pipeline")
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args()
    run(args.input, args.output)


if __name__ == "__main__":
    main()
