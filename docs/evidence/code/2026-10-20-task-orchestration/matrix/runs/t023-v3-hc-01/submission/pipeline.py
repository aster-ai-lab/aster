#!/usr/bin/env python3
"""Global order audit, event ledger, reconciliation and budget optimization (107-v1)."""

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, getcontext

getcontext().prec = 60

SCHEMA_VERSION = "107-v1"
EVENT_TYPES = ("cancel", "refund", "shipping_credit")
HIGH_DISCOUNT_PPM = 200000


def parse_scaled(text, scale):
    if text is None:
        return 0
    s = str(text).strip()
    if s == "":
        return 0
    value = Decimal(s) * scale
    if value == value.to_integral_value():
        return int(value)
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def parse_int(text):
    if text is None:
        return 0
    s = str(text).strip()
    if s == "":
        return 0
    return int(Decimal(s))


def round_div(numerator, denominator):
    """ROUND_HALF_UP integer division; None when the denominator is zero."""
    if denominator == 0:
        return None
    if denominator < 0:
        numerator = -numerator
        denominator = -denominator
    sign = -1 if numerator < 0 else 1
    n = abs(numerator)
    q, r = divmod(n, denominator)
    if 2 * r >= denominator:
        q += 1
    return sign * q


def fmt_scaled(value, places):
    if value == 0:
        value = 0
    sign = "-" if value < 0 else ""
    digits = str(abs(value))
    if places == 0:
        return sign + digits
    digits = digits.rjust(places + 1, "0")
    return sign + digits[:-places] + "." + digits[-places:]


def format_reference(value, places):
    s = "" if value is None else str(value).strip()
    if s == "":
        return ""
    try:
        number = Decimal(s)
    except Exception:
        return s
    quantum = Decimal(1).scaleb(-places)
    rounded = number.quantize(quantum, rounding=ROUND_HALF_UP)
    if rounded == 0:
        rounded = Decimal(0).quantize(quantum)
    return f"{rounded:.{places}f}"


def market_from_filename(filename):
    base = os.path.basename(filename)
    if base.lower().endswith(".csv"):
        base = base[:-4]
    if base.endswith("_orders"):
        base = base[:-7]
    return base


def read_product_catalog(path):
    catalog = {}
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            pid = (row.get("Product ID") or "").strip()
            if pid == "":
                continue
            catalog[pid] = (
                (row.get("Category") or "").strip(),
                (row.get("Sub-Category") or "").strip(),
            )
    return catalog


def read_orders_file(path, catalog):
    name = os.path.basename(path)
    market = market_from_filename(name)
    rows = []
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        try:
            header = next(reader)
        except StopIteration:
            return rows
        header = [cell.strip() for cell in header]
        position = {field: index for index, field in enumerate(header)}
        sequence = 0
        for raw in reader:
            if not raw or all((cell or "").strip() == "" for cell in raw):
                continue
            sequence += 1

            def cell(field):
                index = position[field]
                if index >= len(raw):
                    return ""
                return raw[index].strip()

            product_id = cell("Product ID")
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date = cell("Order Date")
            ship_date = cell("Ship Date")
            delivery_days = (date.fromisoformat(ship_date) - date.fromisoformat(order_date)).days
            row_key = name + ":" + str(sequence).zfill(6)
            rows.append({
                "row_key": row_key,
                "source_file": name,
                "source_row": sequence,
                "order_id": cell("Order ID"),
                "product_id": product_id,
                "market": market,
                "category": category,
                "sub_category": sub_category,
                "segment": cell("Segment"),
                "ship_mode": cell("Ship Mode"),
                "priority": cell("Order Priority"),
                "order_date": order_date,
                "ship_date": ship_date,
                "delivery_days": delivery_days,
                "quantity": parse_int(cell("Quantity")),
                "sales_milli": parse_scaled(cell("Sales"), 1000),
                "profit_milli": parse_scaled(cell("Profit"), 1000),
                "shipping_milli": parse_scaled(cell("Shipping Cost"), 1000),
                "discount_ppm": parse_scaled(cell("Discount"), 1000000),
            })
    return rows


def load_json(path):
    with open(path, "r", encoding="utf-8-sig") as handle:
        return json.load(handle)


def load_events(path):
    records = []
    with open(path, "r", encoding="utf-8-sig") as handle:
        for line in handle:
            if line.strip() == "":
                continue
            records.append(json.loads(line))
    return records


def process_events(normalized_rows, records):
    by_key = {row["row_key"]: row for row in normalized_rows}
    effective = {key: dict(row, active=True) for key, row in by_key.items()}

    seen = set()
    kept = []
    meta = []
    status_by_line = {}
    for line_no, event in enumerate(records, 1):
        event_id = event.get("event_id")
        row_key = event.get("row_key")
        meta.append((line_no, event_id, row_key))
        if event_id in seen:
            status_by_line[line_no] = "duplicate_event"
        else:
            seen.add(event_id)
            kept.append((line_no, event))

    kept.sort(key=lambda item: (str(item[1].get("timestamp", "")), str(item[1].get("event_id", ""))))

    for line_no, event in kept:
        row_key = event.get("row_key")
        event_type = event.get("type")
        amount = event.get("amount_milli")
        if row_key not in effective:
            status = "unknown_row"
        elif event_type not in EVENT_TYPES:
            status = "unknown_type"
        elif not effective[row_key]["active"]:
            status = "order_cancelled"
        else:
            amount_is_int = isinstance(amount, int) and not isinstance(amount, bool)
            row = effective[row_key]
            if event_type == "cancel":
                if not (amount_is_int and amount == 0):
                    status = "invalid_amount"
                else:
                    row["active"] = False
                    row["sales_milli"] = 0
                    row["profit_milli"] = 0
                    row["shipping_milli"] = 0
                    status = "applied"
            elif event_type == "refund":
                if not (amount_is_int and amount > 0 and amount <= row["sales_milli"]):
                    status = "invalid_amount"
                else:
                    row["sales_milli"] -= amount
                    row["profit_milli"] -= amount
                    status = "applied"
            else:
                if not (amount_is_int and amount > 0 and amount <= row["shipping_milli"]):
                    status = "invalid_amount"
                else:
                    row["shipping_milli"] -= amount
                    status = "applied"
        status_by_line[line_no] = status

    audit = [
        {
            "input_line": line_no,
            "event_id": event_id,
            "row_key": row_key,
            "status": status_by_line[line_no],
        }
        for line_no, event_id, row_key in meta
    ]
    effective_rows = [effective[key] for key in sorted(effective)]
    return audit, effective_rows


def new_metrics():
    return {
        "row_count": 0,
        "quantity": 0,
        "sales_milli": 0,
        "profit_milli": 0,
        "shipping_milli": 0,
        "high_discount_count": 0,
        "negative_contribution_count": 0,
    }


def accumulate(metrics, row):
    metrics["row_count"] += 1
    metrics["quantity"] += row["quantity"]
    metrics["sales_milli"] += row["sales_milli"]
    metrics["profit_milli"] += row["profit_milli"]
    metrics["shipping_milli"] += row["shipping_milli"]
    if row["discount_ppm"] > HIGH_DISCOUNT_PPM:
        metrics["high_discount_count"] += 1
    if row["profit_milli"] < row["shipping_milli"]:
        metrics["negative_contribution_count"] += 1


def finalize_metrics(metrics):
    result = dict(metrics)
    result["contribution_milli"] = metrics["profit_milli"] - metrics["shipping_milli"]
    result["profit_margin_bp"] = (
        None if metrics["sales_milli"] == 0
        else round_div(metrics["profit_milli"] * 10000, metrics["sales_milli"])
    )
    result["contribution_margin_bp"] = (
        None if metrics["sales_milli"] == 0
        else round_div(result["contribution_milli"] * 10000, metrics["sales_milli"])
    )
    return result


GROUP_DEFINITIONS = [
    ("market", ("market",)),
    ("market_category", ("market", "category")),
    ("segment", ("segment",)),
    ("ship_mode", ("ship_mode",)),
    ("market_month", ("market", "month")),
]


def group_key(row, fields):
    return tuple(
        row["order_date"][:7] if field == "month" else row[field]
        for field in fields
    )


def build_aggregates(normalized_rows, effective_rows):
    groups = {name: {} for name, _ in GROUP_DEFINITIONS}
    for row in normalized_rows:
        for name, fields in GROUP_DEFINITIONS:
            key = group_key(row, fields)
            groups[name].setdefault(key, new_metrics())
    for row in effective_rows:
        if not row["active"]:
            continue
        for name, fields in GROUP_DEFINITIONS:
            accumulate(groups[name][group_key(row, fields)], row)

    aggregates = {}
    for name, fields in GROUP_DEFINITIONS:
        items = []
        for key in sorted(groups[name].keys()):
            obj = {fields[index]: key[index] for index in range(len(fields))}
            obj.update(finalize_metrics(groups[name][key]))
            items.append(obj)
        aggregates[name] = items
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    metrics = new_metrics()
    for row in effective_rows:
        if row["active"]:
            accumulate(metrics, row)
    summary = finalize_metrics(metrics)
    summary["input_row_count"] = len(normalized_rows)
    unique_orders = len({row["order_id"] for row in normalized_rows})
    summary["unique_order_id_count"] = unique_orders
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - unique_orders
    summary["cancelled_row_count"] = sum(1 for row in effective_rows if not row["active"])
    status_counts = {}
    for item in audit:
        status = item["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    summary["event_status_counts"] = status_counts
    return summary


def read_reference_rows(path):
    rows = []
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            cleaned = {}
            empty = True
            for key, value in row.items():
                if key is None:
                    continue
                cleaned[key.strip()] = value.strip() if isinstance(value, str) else value
                if cleaned[key.strip()] != "":
                    empty = False
            if not empty:
                rows.append(cleaned)
    return rows


SEGMENT_DECIMALS = {
    "Order_Count": 0,
    "Total_Sales": 2,
    "Total_Profit": 2,
    "Profit_Margin": 2,
    "Avg_Discount": 4,
}
SHIPPING_DECIMALS = {
    "Order_Count": 0,
    "Total_Shipping_Cost": 2,
    "Avg_Shipping_Cost": 2,
    "Avg_Delivery_Days": 2,
}


def build_reconciliation(normalized_rows, input_dir):
    segment_stats = {}
    shipping_stats = {}
    for row in normalized_rows:
        seg = segment_stats.setdefault(row["segment"], [0, 0, 0, 0])
        seg[0] += 1
        seg[1] += row["sales_milli"]
        seg[2] += row["profit_milli"]
        seg[3] += row["discount_ppm"]
        ship = shipping_stats.setdefault(row["ship_mode"], [0, 0, 0])
        ship[0] += 1
        ship[1] += row["shipping_milli"]
        ship[2] += row["delivery_days"]

    output = []

    segment_path = os.path.join(input_dir, "segment_performance.csv")
    if os.path.isfile(segment_path):
        for reference_row in read_reference_rows(segment_path):
            group = reference_row.get("Segment", "")
            stats = segment_stats.get(group)
            count = stats[0] if stats else 0
            sales = stats[1] if stats else 0
            profit = stats[2] if stats else 0
            discount = stats[3] if stats else 0
            actuals = {
                "Order_Count": fmt_scaled(round_div(count, 1), 0),
                "Total_Sales": fmt_scaled(round_div(sales, 10), 2),
                "Total_Profit": fmt_scaled(round_div(profit, 10), 2),
                "Profit_Margin": fmt_scaled(
                    round_div(profit * 10000, sales) if sales != 0 else 0, 2
                ),
                "Avg_Discount": fmt_scaled(
                    round_div(discount, count * 100) if count else 0, 4
                ),
            }
            for field, places in SEGMENT_DECIMALS.items():
                if field not in reference_row:
                    continue
                actual = actuals[field]
                reference = format_reference(reference_row.get(field), places)
                output.append({
                    "source_file": "segment_performance.csv",
                    "group": group,
                    "field": field,
                    "actual": actual,
                    "reference": reference,
                    "match": actual == reference,
                })

    shipping_path = os.path.join(input_dir, "shipping_by_mode.csv")
    if os.path.isfile(shipping_path):
        for reference_row in read_reference_rows(shipping_path):
            group = reference_row.get("Ship_Mode", "")
            stats = shipping_stats.get(group)
            count = stats[0] if stats else 0
            shipping = stats[1] if stats else 0
            delivery_days = stats[2] if stats else 0
            actuals = {
                "Order_Count": fmt_scaled(round_div(count, 1), 0),
                "Total_Shipping_Cost": fmt_scaled(round_div(shipping, 10), 2),
                "Avg_Shipping_Cost": fmt_scaled(
                    round_div(shipping, count * 10) if count else 0, 2
                ),
                "Avg_Delivery_Days": fmt_scaled(
                    round_div(delivery_days * 100, count) if count else 0, 2
                ),
            }
            for field, places in SHIPPING_DECIMALS.items():
                if field not in reference_row:
                    continue
                actual = actuals[field]
                reference = format_reference(reference_row.get(field), places)
                output.append({
                    "source_file": "shipping_by_mode.csv",
                    "group": group,
                    "field": field,
                    "actual": actual,
                    "reference": reference,
                    "match": actual == reference,
                })

    output.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return output


def build_candidates(effective_rows, policy):
    shipping_priorities = set(policy.get("shipping_priorities", []))
    shipping_saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    discount_recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_costs = policy.get("shipping_cost_by_market", {})
    discount_costs = policy.get("discount_cost_by_market", {})
    candidates = []
    for row in effective_rows:
        if not row["active"]:
            continue
        row_key = row["row_key"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in shipping_priorities:
            gain = (row["shipping_milli"] * shipping_saving_bp) // 10000
            if gain > 0:
                candidates.append({
                    "action_id": row_key + "|shipping_switch",
                    "row_key": row_key,
                    "type": "shipping_switch",
                    "cost_units": int(shipping_costs[row["market"]]),
                    "gain_milli": gain,
                })
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold_ppm and contribution < 0:
            gain = (row["sales_milli"] * discount_recovery_bp) // 10000
            if gain > 0:
                candidates.append({
                    "action_id": row_key + "|discount_review",
                    "row_key": row_key,
                    "type": "discount_review",
                    "cost_units": int(discount_costs[row["market"]]),
                    "gain_milli": gain,
                })
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimize(candidates, budget):
    groups = {}
    for candidate in candidates:
        groups.setdefault(candidate["row_key"], []).append(candidate)

    states = {0: (0, ())}
    for row_key in sorted(groups.keys()):
        options = groups[row_key]
        updated = dict(states)
        for cost, (gain, action_ids) in states.items():
            for candidate in options:
                new_cost = cost + candidate["cost_units"]
                if new_cost > budget:
                    continue
                new_gain = gain + candidate["gain_milli"]
                new_ids = tuple(sorted(action_ids + (candidate["action_id"],)))
                current = updated.get(new_cost)
                if (
                    current is None
                    or new_gain > current[0]
                    or (new_gain == current[0] and new_ids < current[1])
                ):
                    updated[new_cost] = (new_gain, new_ids)
        pruned = {}
        best_gain = -1
        for cost in sorted(updated.keys()):
            gain, action_ids = updated[cost]
            if gain > best_gain:
                pruned[cost] = (gain, action_ids)
                best_gain = gain
        states = pruned

    if not states:
        return 0, 0, ()
    top_gain = max(gain for gain, _ in states.values())
    top_cost = min(cost for cost, (gain, _) in states.items() if gain == top_gain)
    top_ids = min(
        action_ids
        for cost, (gain, action_ids) in states.items()
        if gain == top_gain and cost == top_cost
    )
    return top_cost, top_gain, top_ids


def sha256_map(input_dir):
    result = {}
    with os.scandir(input_dir) as entries:
        for entry in entries:
            if not entry.is_file():
                continue
            digest = hashlib.sha256()
            with open(entry.path, "rb") as handle:
                while True:
                    chunk = handle.read(1024 * 1024)
                    if not chunk:
                        break
                    digest.update(chunk)
            result[entry.name] = digest.hexdigest()
    return result


def build_results(input_dir):
    catalog = read_product_catalog(os.path.join(input_dir, "product_info.csv"))
    order_files = sorted(
        entry.path
        for entry in os.scandir(input_dir)
        if entry.is_file() and entry.name.endswith("_orders.csv")
    )
    normalized_rows = []
    for path in order_files:
        normalized_rows.extend(read_orders_file(path, catalog))
    normalized_rows.sort(key=lambda row: row["row_key"])

    policy = load_json(os.path.join(input_dir, "policy.json"))
    records = load_events(os.path.join(input_dir, "adjustments.jsonl"))
    audit, effective_rows = process_events(normalized_rows, records)
    aggregates = build_aggregates(normalized_rows, effective_rows)
    reconciliation = build_reconciliation(normalized_rows, input_dir)
    summary = build_summary(normalized_rows, effective_rows, audit)

    candidates = build_candidates(effective_rows, policy)
    budget = int(policy.get("budget_units", 0))
    spent_units, gain_milli, selected_ids = optimize(candidates, budget)
    candidate_by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected_actions = [candidate_by_id[action_id] for action_id in selected_ids]
    optimization = {
        "budget_units": budget,
        "spent_units": spent_units,
        "gain_milli": gain_milli,
        "selected_action_ids": list(selected_ids),
        "projected_contribution_milli": summary["contribution_milli"] + gain_milli,
    }

    return {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized_rows,
        "event_audit": audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected_actions,
        "optimization": optimization,
        "input_sha256": sha256_map(input_dir),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description="107-v1 order audit pipeline")
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    results = build_results(args.input)
    os.makedirs(args.output, exist_ok=True)
    output_path = os.path.join(args.output, "results.json")
    text = json.dumps(results, ensure_ascii=False, sort_keys=True, indent=2)
    with open(output_path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
        handle.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
