#!/usr/bin/env python3
"""107-V1 global order audit, event processing and budget optimisation.

Usage:
    python pipeline.py --input input --output output

Only the Python standard library is used.  The result is fully deterministic:
no timestamps, absolute paths or random values are written.
"""

import argparse
import csv
import hashlib
import json
import os
import sys
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, getcontext

getcontext().prec = 60

SCHEMA_VERSION = "107-v1"
ORDERS_SUFFIX = "_orders.csv"
VALID_EVENT_TYPES = ("cancel", "refund", "shipping_credit")
HIGH_DISCOUNT_PPM = 200000
MILLI = Decimal(1000)
PPM = Decimal(1000000)

# ---------------------------------------------------------------------------
# numeric helpers
# ---------------------------------------------------------------------------


def parse_scaled(text, scale):
    """Exact conversion of a decimal string to a scaled integer."""
    s = (text or "").strip()
    if not s:
        return 0
    value = Decimal(s) * scale
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def parse_milli(text):
    return parse_scaled(text, MILLI)


def parse_ppm(text):
    return parse_scaled(text, PPM)


def parse_int(text):
    s = (text or "").strip()
    if not s:
        return 0
    return int(Decimal(s).to_integral_value(rounding=ROUND_HALF_UP))


def ratio_bp(numerator, denominator):
    """ROUND_HALF_UP basis points, or None when the denominator is zero."""
    if denominator == 0:
        return None
    value = (Decimal(numerator) * 10000) / Decimal(denominator)
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def fmt_fixed(value, places):
    """Format a Decimal with a fixed number of places (ROUND_HALF_UP)."""
    quantum = Decimal(1).scaleb(-places)
    quantized = Decimal(value).quantize(quantum, rounding=ROUND_HALF_UP)
    if quantized == 0:
        quantized = quantized.copy_abs()
    return format(quantized, ".%df" % places)


# ---------------------------------------------------------------------------
# CSV loading
# ---------------------------------------------------------------------------


def read_csv(path):
    """Return (header, records) skipping blank lines."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        try:
            header = next(reader)
        except StopIteration:
            return [], []
        header = [column.strip() for column in header]
        records = []
        for raw in reader:
            if not raw:
                continue
            if all(cell.strip() == "" for cell in raw):
                continue
            records.append(dict(zip(header, raw)))
    return header, records


def load_products(input_dir):
    path = os.path.join(input_dir, "product_info.csv")
    products = {}
    if not os.path.isfile(path):
        return products
    _, records = read_csv(path)
    for record in records:
        product_id = (record.get("Product ID") or "").strip()
        products[product_id] = (
            (record.get("Category") or "").strip(),
            (record.get("Sub-Category") or "").strip(),
        )
    return products


def load_policy(input_dir):
    path = os.path.join(input_dir, "policy.json")
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def load_event_records(input_dir):
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


# ---------------------------------------------------------------------------
# normalisation
# ---------------------------------------------------------------------------


def build_normalized_rows(input_dir):
    products = load_products(input_dir)
    filenames = sorted(
        name
        for name in os.listdir(input_dir)
        if name.endswith(ORDERS_SUFFIX)
        and os.path.isfile(os.path.join(input_dir, name))
    )
    rows = []
    for filename in filenames:
        market = filename[: -len(ORDERS_SUFFIX)]
        _, records = read_csv(os.path.join(input_dir, filename))
        for index, record in enumerate(records, 1):
            product_id = (record.get("Product ID") or "").strip()
            category, sub_category = products.get(product_id, ("", ""))
            order_date = (record.get("Order Date") or "").strip()
            ship_date = (record.get("Ship Date") or "").strip()
            delivery_days = (date.fromisoformat(ship_date) - date.fromisoformat(order_date)).days
            rows.append(
                {
                    "row_key": "%s:%06d" % (filename, index),
                    "source_file": filename,
                    "source_row": index,
                    "order_id": (record.get("Order ID") or "").strip(),
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": (record.get("Segment") or "").strip(),
                    "ship_mode": (record.get("Ship Mode") or "").strip(),
                    "priority": (record.get("Order Priority") or "").strip(),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": delivery_days,
                    "quantity": parse_int(record.get("Quantity")),
                    "sales_milli": parse_milli(record.get("Sales")),
                    "profit_milli": parse_milli(record.get("Profit")),
                    "shipping_milli": parse_milli(record.get("Shipping Cost")),
                    "discount_ppm": parse_ppm(record.get("Discount")),
                }
            )
    rows.sort(key=lambda row: row["row_key"])
    return rows


# ---------------------------------------------------------------------------
# event ledger
# ---------------------------------------------------------------------------


def is_json_int(value):
    return isinstance(value, int) and not isinstance(value, bool)


def apply_events(rows, event_records):
    """Return (event_audit, effective_rows) after applying the ledger."""
    state = {}
    for row in rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    seen_ids = set()
    entries = []  # (input_line, event, is_duplicate)
    for line_no, event in enumerate(event_records, 1):
        event_id = event.get("event_id")
        duplicate = event_id in seen_ids
        if not duplicate:
            seen_ids.add(event_id)
        entries.append((line_no, event, duplicate))

    retained = [(line_no, event) for line_no, event, dup in entries if not dup]
    retained.sort(key=lambda item: (str(item[1].get("timestamp")), str(item[1].get("event_id"))))

    statuses = {}
    for line_no, event in retained:
        row_key = event.get("row_key")
        target = state.get(row_key)
        if target is None:
            statuses[line_no] = "unknown_row"
            continue
        event_type = event.get("type")
        if event_type not in VALID_EVENT_TYPES:
            statuses[line_no] = "unknown_type"
            continue
        if not target["active"]:
            statuses[line_no] = "order_cancelled"
            continue
        amount = event.get("amount_milli")
        if event_type == "cancel":
            if not is_json_int(amount) or amount != 0:
                statuses[line_no] = "invalid_amount"
                continue
            target["active"] = False
            target["sales_milli"] = 0
            target["profit_milli"] = 0
            target["shipping_milli"] = 0
            statuses[line_no] = "applied"
        elif event_type == "refund":
            if not is_json_int(amount) or amount <= 0 or amount > target["sales_milli"]:
                statuses[line_no] = "invalid_amount"
                continue
            target["sales_milli"] -= amount
            target["profit_milli"] -= amount
            statuses[line_no] = "applied"
        else:  # shipping_credit
            if not is_json_int(amount) or amount <= 0 or amount > target["shipping_milli"]:
                statuses[line_no] = "invalid_amount"
                continue
            target["shipping_milli"] -= amount
            statuses[line_no] = "applied"

    audit = []
    for line_no, event, duplicate in entries:
        audit.append(
            {
                "input_line": line_no,
                "event_id": event.get("event_id"),
                "row_key": event.get("row_key"),
                "status": "duplicate_event" if duplicate else statuses[line_no],
            }
        )

    effective = []
    for row in rows:
        current = state[row["row_key"]]
        updated = dict(row)
        updated["sales_milli"] = current["sales_milli"]
        updated["profit_milli"] = current["profit_milli"]
        updated["shipping_milli"] = current["shipping_milli"]
        updated["active"] = current["active"]
        effective.append(updated)
    effective.sort(key=lambda row: row["row_key"])
    return audit, effective


# ---------------------------------------------------------------------------
# metrics and aggregates
# ---------------------------------------------------------------------------


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
        "profit_margin_bp": ratio_bp(profit, sales),
        "contribution_margin_bp": ratio_bp(contribution, sales),
        "high_discount_count": sum(
            1 for row in rows if row["discount_ppm"] > HIGH_DISCOUNT_PPM
        ),
        "negative_contribution_count": sum(
            1 for row in rows if row["profit_milli"] < row["shipping_milli"]
        ),
    }


AGGREGATE_DEFS = (
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


def build_aggregates(normalized_rows, effective_rows):
    aggregates = {}
    active_rows = [row for row in effective_rows if row["active"]]
    for name, fields in AGGREGATE_DEFS:
        keys = set()
        for row in normalized_rows:
            keys.add(tuple(group_value(row, field) for field in fields))
        buckets = dict((key, []) for key in keys)
        for row in active_rows:
            key = tuple(group_value(row, field) for field in fields)
            if key in buckets:
                buckets[key].append(row)
        entries = []
        for key in sorted(buckets):
            item = {}
            for field, value in zip(fields, key):
                item[field] = value
            item.update(compute_metrics(buckets[key]))
            entries.append(item)
        aggregates[name] = entries
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    active_rows = [row for row in effective_rows if row["active"]]
    summary = compute_metrics(active_rows)
    order_ids = set(row["order_id"] for row in normalized_rows)
    status_counts = {}
    for entry in audit:
        status = entry["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - len(order_ids)
    summary["cancelled_row_count"] = sum(
        1 for row in effective_rows if not row["active"]
    )
    summary["event_status_counts"] = dict(sorted(status_counts.items()))
    return summary


# ---------------------------------------------------------------------------
# reconciliation against the legacy summary tables
# ---------------------------------------------------------------------------

RECON_FIELD_PLACES = {
    "Order_Count": 0,
    "Total_Sales": 2,
    "Total_Profit": 2,
    "Profit_Margin": 2,
    "Avg_Discount": 4,
    "Total_Shipping_Cost": 2,
    "Avg_Shipping_Cost": 2,
    "Avg_Delivery_Days": 2,
}


def segment_actuals(rows):
    count = len(rows)
    sales = Decimal(sum(row["sales_milli"] for row in rows)) / MILLI
    profit = Decimal(sum(row["profit_milli"] for row in rows)) / MILLI
    discount_sum = Decimal(sum(row["discount_ppm"] for row in rows))
    values = {
        "Order_Count": Decimal(count),
        "Total_Sales": sales,
        "Total_Profit": profit,
        "Profit_Margin": (profit / sales * 100) if sales != 0 else Decimal(0),
        "Avg_Discount": (
            discount_sum / Decimal(count) / PPM if count else Decimal(0)
        ),
    }
    return values


def shipping_actuals(rows):
    count = len(rows)
    total_shipping = Decimal(sum(row["shipping_milli"] for row in rows)) / MILLI
    days_sum = Decimal(sum(row["delivery_days"] for row in rows))
    values = {
        "Order_Count": Decimal(count),
        "Total_Shipping_Cost": total_shipping,
        "Avg_Shipping_Cost": total_shipping / Decimal(count) if count else Decimal(0),
        "Avg_Delivery_Days": days_sum / Decimal(count) if count else Decimal(0),
    }
    return values


RECON_SPECS = (
    ("segment_performance.csv", "segment", segment_actuals),
    ("shipping_by_mode.csv", "ship_mode", shipping_actuals),
)


def build_reconciliation(input_dir, normalized_rows):
    results = []
    for filename, group_field, actuals in RECON_SPECS:
        path = os.path.join(input_dir, filename)
        if not os.path.isfile(path):
            continue
        header, records = read_csv(path)
        if not header:
            continue
        group_column = header[0]
        fields = [column for column in header[1:] if column in RECON_FIELD_PLACES]
        grouped = {}
        for row in normalized_rows:
            grouped.setdefault(row[group_field], []).append(row)
        for record in records:
            group = (record.get(group_column) or "").strip()
            values = actuals(grouped.get(group, []))
            for field in fields:
                places = RECON_FIELD_PLACES[field]
                actual_text = fmt_fixed(values.get(field, Decimal(0)), places)
                reference_text = fmt_fixed(
                    Decimal((record.get(field) or "").strip() or "0"), places
                )
                results.append(
                    {
                        "source_file": filename,
                        "group": group,
                        "field": field,
                        "actual": actual_text,
                        "reference": reference_text,
                        "match": actual_text == reference_text,
                    }
                )
    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


# ---------------------------------------------------------------------------
# candidate actions and exact budget optimisation
# ---------------------------------------------------------------------------


def build_candidates(effective_rows, policy):
    shipping_priorities = set(policy.get("shipping_priorities") or [])
    shipping_saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    discount_recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_cost = policy.get("shipping_cost_by_market") or {}
    discount_cost = policy.get("discount_cost_by_market") or {}

    candidates = []
    for row in effective_rows:
        market = row["market"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in shipping_priorities:
            cost_units = shipping_cost.get(market)
            if cost_units is not None:
                gain = row["shipping_milli"] * shipping_saving_bp // 10000
                if gain > 0:
                    candidates.append(
                        {
                            "action_id": row["row_key"] + "|shipping_switch",
                            "row_key": row["row_key"],
                            "type": "shipping_switch",
                            "cost_units": int(cost_units),
                            "gain_milli": int(gain),
                        }
                    )
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold_ppm and contribution < 0:
            cost_units = discount_cost.get(market)
            if cost_units is not None:
                gain = row["sales_milli"] * discount_recovery_bp // 10000
                if gain > 0:
                    candidates.append(
                        {
                            "action_id": row["row_key"] + "|discount_review",
                            "row_key": row["row_key"],
                            "type": "discount_review",
                            "cost_units": int(cost_units),
                            "gain_milli": int(gain),
                        }
                    )
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def selection_better(left, right):
    """True when (gain, cost, ids) left beats right: max gain, min cost, min ids."""
    if left[0] != right[0]:
        return left[0] > right[0]
    if left[1] != right[1]:
        return left[1] < right[1]
    return left[2] < right[2]


def optimise(candidates, budget_units):
    """Exact DP over row groups; one candidate per row_key at most."""
    grouped = {}
    for candidate in candidates:
        grouped.setdefault(candidate["row_key"], []).append(candidate)

    max_total = sum(
        max(item["cost_units"] for item in group) for group in grouped.values()
    )
    limit = min(budget_units, max_total)
    if limit <= 0 or not grouped:
        return {}, 0

    # states: cost -> (gain, tuple(action_ids) ascending)
    # Groups are processed from the last row_key to the first, so a candidate is
    # always prepended to an already ascending id tuple.  Comparing suffixes and
    # keeping the lexicographically smallest one is order-preserving once an
    # identical prefix is prepended, which makes the per-state pruning exact.
    states = {0: (0, ())}
    for row_key in sorted(grouped, reverse=True):
        group = sorted(grouped[row_key], key=lambda item: item["action_id"])
        updated = dict(states)
        for candidate in group:
            cost = candidate["cost_units"]
            gain = candidate["gain_milli"]
            if cost > limit:
                continue
            for spent, (current_gain, ids) in states.items():
                new_cost = spent + cost
                if new_cost > limit:
                    continue
                new_gain = current_gain + gain
                new_ids = (candidate["action_id"],) + ids
                existing = updated.get(new_cost)
                if existing is None or selection_better(
                    (new_gain, new_cost, new_ids),
                    (existing[0], new_cost, existing[1]),
                ):
                    updated[new_cost] = (new_gain, new_ids)
        states = updated

    best = None
    for spent, (gain, ids) in states.items():
        candidate = (gain, spent, ids)
        if best is None or selection_better(candidate, best):
            best = candidate
    if best is None:
        return {}, 0
    gain, spent, ids = best
    by_id = dict((item["action_id"], item) for item in candidates)
    return dict((action_id, by_id[action_id]) for action_id in ids), gain


# ---------------------------------------------------------------------------
# hashing and output
# ---------------------------------------------------------------------------


def hash_inputs(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path):
            continue
        hasher = hashlib.sha256()
        with open(path, "rb") as handle:
            while True:
                chunk = handle.read(1 << 20)
                if not chunk:
                    break
                hasher.update(chunk)
        digests[name] = hasher.hexdigest()
    return digests


def run(input_dir, output_dir):
    normalized_rows = build_normalized_rows(input_dir)
    event_records = load_event_records(input_dir)
    policy = load_policy(input_dir)

    audit, effective_rows = apply_events(normalized_rows, event_records)
    aggregates = build_aggregates(normalized_rows, effective_rows)
    reconciliation = build_reconciliation(input_dir, normalized_rows)
    summary = build_summary(normalized_rows, effective_rows, audit)

    candidates = build_candidates(effective_rows, policy)
    budget_units = int(policy.get("budget_units", 0))
    selected_map, total_gain = optimise(candidates, budget_units)
    selected = [selected_map[key] for key in sorted(selected_map)]
    spent = sum(item["cost_units"] for item in selected)

    results = {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized_rows,
        "event_audit": audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected,
        "optimization": {
            "budget_units": budget_units,
            "spent_units": spent,
            "gain_milli": total_gain,
            "selected_action_ids": [item["action_id"] for item in selected],
            "projected_contribution_milli": summary["contribution_milli"] + total_gain,
        },
        "input_sha256": hash_inputs(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    target = os.path.join(output_dir, "results.json")
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, indent=2, allow_nan=False)
        handle.write("\n")
    return target


def main(argv=None):
    parser = argparse.ArgumentParser(description="107-V1 order audit pipeline")
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args(argv)
    run(args.input, args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
