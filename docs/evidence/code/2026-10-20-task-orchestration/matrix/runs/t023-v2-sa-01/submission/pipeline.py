#!/usr/bin/env python3
"""Global order audit, event processing and budget optimization (107-V1).

Standard library only.  Run as:

    python pipeline.py --input input --output output
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import sys
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, localcontext

ORDERS_SUFFIX = "_orders.csv"
PRODUCT_FILE = "product_info.csv"
SEGMENT_FILE = "segment_performance.csv"
SHIPPING_FILE = "shipping_by_mode.csv"
EVENTS_FILE = "adjustments.jsonl"
POLICY_FILE = "policy.json"
SCHEMA_VERSION = "107-v1"
HIGH_DISCOUNT_PPM = 200000
VALID_EVENT_TYPES = ("cancel", "refund", "shipping_credit")

COMMON_METRIC_FIELDS = (
    "row_count",
    "quantity",
    "sales_milli",
    "profit_milli",
    "shipping_milli",
    "contribution_milli",
    "profit_margin_bp",
    "contribution_margin_bp",
    "high_discount_count",
    "negative_contribution_count",
)


# --------------------------------------------------------------------------
# numeric helpers (exact decimal -> integer scaling, half-up rounding)
# --------------------------------------------------------------------------
def _decimal(text):
    s = "" if text is None else str(text).strip()
    if not s:
        return Decimal(0)
    return Decimal(s)


def scaled_int(text, places):
    """Exact scaling of a decimal string by 10**places, ROUND_HALF_UP."""
    value = _decimal(text) * (Decimal(10) ** places)
    with localcontext() as ctx:
        ctx.prec = 60
        return int(value.quantize(Decimal(1), rounding=ROUND_HALF_UP))


def as_int(text):
    return scaled_int(text, 0)


def ratio_bp(numerator, denominator):
    """numerator / denominator * 10000 as an integer basis point value."""
    if denominator == 0:
        return None
    with localcontext() as ctx:
        ctx.prec = 60
        value = Decimal(numerator) / Decimal(denominator) * Decimal(10000)
        return int(value.quantize(Decimal(1), rounding=ROUND_HALF_UP))


def fixed_string(value, places):
    """Format a Decimal with exactly `places` decimals using ROUND_HALF_UP."""
    quantum = Decimal(1).scaleb(-places)
    with localcontext() as ctx:
        ctx.prec = 60
        rounded = value.quantize(quantum, rounding=ROUND_HALF_UP)
    if rounded == 0:
        rounded = abs(rounded)
    return f"{rounded:.{places}f}"


def divide(numerator, denominator):
    with localcontext() as ctx:
        ctx.prec = 60
        return Decimal(numerator) / Decimal(denominator)


# --------------------------------------------------------------------------
# input loading
# --------------------------------------------------------------------------
def read_csv_records(path):
    """Return (header, records) skipping blank rows; keeps every data record."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        header = None
        records = []
        for row in reader:
            if header is None:
                header = [cell.strip() for cell in row]
                continue
            if not row or all(cell.strip() == "" for cell in row):
                continue
            if len(row) < len(header):
                row = row + [""] * (len(header) - len(row))
            records.append(dict(zip(header, row)))
    return header or [], records


def load_catalog(input_dir):
    catalog = {}
    path = os.path.join(input_dir, PRODUCT_FILE)
    if not os.path.exists(path):
        return catalog
    _, records = read_csv_records(path)
    for rec in records:
        pid = rec.get("Product ID", "").strip()
        if pid and pid not in catalog:
            catalog[pid] = (
                rec.get("Category", "").strip(),
                rec.get("Sub-Category", "").strip(),
            )
    return catalog


def order_files(input_dir):
    names = []
    for name in os.listdir(input_dir):
        if not name.endswith(ORDERS_SUFFIX):
            continue
        if not os.path.isfile(os.path.join(input_dir, name)):
            continue
        names.append(name)
    return sorted(names)


def normalize_rows(input_dir, catalog):
    rows = []
    for name in order_files(input_dir):
        market = name[: -len(ORDERS_SUFFIX)]
        _, records = read_csv_records(os.path.join(input_dir, name))
        for index, rec in enumerate(records, start=1):
            product_id = rec.get("Product ID", "").strip()
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date = rec.get("Order Date", "").strip()
            ship_date = rec.get("Ship Date", "").strip()
            rows.append(
                {
                    "row_key": f"{name}:{index:06d}",
                    "source_file": name,
                    "source_row": index,
                    "order_id": rec.get("Order ID", "").strip(),
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": rec.get("Segment", "").strip(),
                    "ship_mode": rec.get("Ship Mode", "").strip(),
                    "priority": rec.get("Order Priority", "").strip(),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": (
                        date.fromisoformat(ship_date) - date.fromisoformat(order_date)
                    ).days,
                    "quantity": as_int(rec.get("Quantity", "")),
                    "sales_milli": scaled_int(rec.get("Sales", ""), 3),
                    "profit_milli": scaled_int(rec.get("Profit", ""), 3),
                    "shipping_milli": scaled_int(rec.get("Shipping Cost", ""), 3),
                    "discount_ppm": scaled_int(rec.get("Discount", ""), 6),
                }
            )
    rows.sort(key=lambda r: r["row_key"])
    return rows


# --------------------------------------------------------------------------
# events
# --------------------------------------------------------------------------
def load_events(path):
    """Non-blank JSONL records in file order, numbered from 1."""
    events = []
    if not os.path.exists(path):
        return events
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            events.append(json.loads(line))
    return events


def is_int_amount(value):
    return isinstance(value, int) and not isinstance(value, bool)


def process_events(rows, raw_events):
    """Apply deduplicated events; return (audit, effective_rows)."""
    state = {}
    for row in rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    seen = set()
    kept = []  # (input_line, event)
    statuses = {}
    for line_no, event in enumerate(raw_events, start=1):
        event_id = event.get("event_id")
        if event_id in seen:
            statuses[line_no] = "duplicate_event"
            continue
        seen.add(event_id)
        kept.append((line_no, event))

    ordered = sorted(kept, key=lambda item: (item[1].get("timestamp", ""), str(item[1].get("event_id"))))

    for line_no, event in ordered:
        row_key = event.get("row_key")
        event_type = event.get("type")
        amount = event.get("amount_milli")

        if row_key not in state:
            statuses[line_no] = "unknown_row"
            continue
        current = state[row_key]
        if event_type not in VALID_EVENT_TYPES:
            statuses[line_no] = "unknown_type"
            continue
        if not current["active"]:
            statuses[line_no] = "order_cancelled"
            continue

        if event_type == "cancel":
            valid = is_int_amount(amount) and amount == 0
        elif event_type == "refund":
            valid = (
                is_int_amount(amount)
                and amount > 0
                and amount <= current["sales_milli"]
            )
        else:  # shipping_credit
            valid = (
                is_int_amount(amount)
                and amount > 0
                and amount <= current["shipping_milli"]
            )

        if not valid:
            statuses[line_no] = "invalid_amount"
            continue

        statuses[line_no] = "applied"
        if event_type == "cancel":
            current["active"] = False
            current["sales_milli"] = 0
            current["profit_milli"] = 0
            current["shipping_milli"] = 0
        elif event_type == "refund":
            current["sales_milli"] -= amount
            current["profit_milli"] -= amount
        else:
            current["shipping_milli"] -= amount

    audit = []
    for line_no, event in enumerate(raw_events, start=1):
        audit.append(
            {
                "input_line": line_no,
                "event_id": event.get("event_id"),
                "row_key": event.get("row_key"),
                "status": statuses[line_no],
            }
        )

    effective = []
    for row in rows:
        updated = dict(row)
        current = state[row["row_key"]]
        updated["sales_milli"] = current["sales_milli"]
        updated["profit_milli"] = current["profit_milli"]
        updated["shipping_milli"] = current["shipping_milli"]
        updated["active"] = current["active"]
        effective.append(updated)
    effective.sort(key=lambda r: r["row_key"])
    return audit, effective


# --------------------------------------------------------------------------
# metrics and aggregates
# --------------------------------------------------------------------------
def compute_metrics(rows):
    row_count = len(rows)
    quantity = sum(r["quantity"] for r in rows)
    sales = sum(r["sales_milli"] for r in rows)
    profit = sum(r["profit_milli"] for r in rows)
    shipping = sum(r["shipping_milli"] for r in rows)
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
        "high_discount_count": sum(1 for r in rows if r["discount_ppm"] > HIGH_DISCOUNT_PPM),
        "negative_contribution_count": sum(
            1 for r in rows if r["profit_milli"] < r["shipping_milli"]
        ),
    }


def build_aggregates(all_rows, active_rows):
    specs = (
        ("market", ("market",)),
        ("market_category", ("market", "category")),
        ("segment", ("segment",)),
        ("ship_mode", ("ship_mode",)),
        ("market_month", ("market", "month")),
    )

    def key_of(row, fields):
        values = []
        for field in fields:
            if field == "month":
                values.append(row["order_date"][:7])
            else:
                values.append(row[field])
        return tuple(values)

    aggregates = {}
    for name, fields in specs:
        keys = set()
        for row in all_rows:
            keys.add(key_of(row, fields))
        buckets = {key: [] for key in keys}
        for row in active_rows:
            key = key_of(row, fields)
            if key in buckets:
                buckets[key].append(row)
        entries = []
        for key in sorted(keys):
            entry = {}
            for field, value in zip(fields, key):
                entry[field] = value
            entry.update(compute_metrics(buckets[key]))
            entries.append(entry)
        aggregates[name] = entries
    return aggregates


def build_summary(all_rows, effective_rows, audit):
    metrics = compute_metrics([r for r in effective_rows if r["active"]])
    order_ids = {r["order_id"] for r in all_rows}
    status_counts = {}
    for entry in audit:
        status = entry["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    summary = {
        "row_count": metrics["row_count"],
        "quantity": metrics["quantity"],
        "sales_milli": metrics["sales_milli"],
        "profit_milli": metrics["profit_milli"],
        "shipping_milli": metrics["shipping_milli"],
        "contribution_milli": metrics["contribution_milli"],
        "profit_margin_bp": metrics["profit_margin_bp"],
        "contribution_margin_bp": metrics["contribution_margin_bp"],
        "high_discount_count": metrics["high_discount_count"],
        "negative_contribution_count": metrics["negative_contribution_count"],
        "input_row_count": len(all_rows),
        "unique_order_id_count": len(order_ids),
        "duplicate_order_id_extra_rows": len(all_rows) - len(order_ids),
        "cancelled_row_count": sum(1 for r in effective_rows if not r["active"]),
        "event_status_counts": {k: status_counts[k] for k in sorted(status_counts)},
    }
    return summary


# --------------------------------------------------------------------------
# reconciliation
# --------------------------------------------------------------------------
def group_rows(rows, field):
    buckets = {}
    for row in rows:
        buckets.setdefault(row[field], []).append(row)
    return buckets


def reconcile_segment(input_dir, rows):
    path = os.path.join(input_dir, SEGMENT_FILE)
    header, records = read_csv_records(path)
    fields = [h for h in header if h != "Segment"]
    buckets = group_rows(rows, "segment")
    output = []
    for rec in records:
        group = rec.get("Segment", "").strip()
        members = buckets.get(group, [])
        count = len(members)
        sales = sum(r["sales_milli"] for r in members)
        profit = sum(r["profit_milli"] for r in members)
        discount = sum(r["discount_ppm"] for r in members)
        actual = {
            "Order_Count": str(count),
            "Total_Sales": fixed_string(divide(sales, 1000), 2),
            "Total_Profit": fixed_string(divide(profit, 1000), 2),
            "Profit_Margin": (
                fixed_string(divide(profit, sales) * 100, 2)
                if sales != 0
                else fixed_string(Decimal(0), 2)
            ),
            "Avg_Discount": (
                fixed_string(divide(discount, count * 1000000), 4)
                if count
                else fixed_string(Decimal(0), 4)
            ),
        }
        decimals = {
            "Order_Count": 0,
            "Total_Sales": 2,
            "Total_Profit": 2,
            "Profit_Margin": 2,
            "Avg_Discount": 4,
        }
        for field in fields:
            output.append(
                _reconciliation_entry(
                    SEGMENT_FILE, group, field, actual[field], rec.get(field, ""), decimals[field]
                )
            )
    return output


def reconcile_shipping(input_dir, rows):
    path = os.path.join(input_dir, SHIPPING_FILE)
    header, records = read_csv_records(path)
    fields = [h for h in header if h != "Ship_Mode"]
    buckets = group_rows(rows, "ship_mode")
    output = []
    for rec in records:
        group = rec.get("Ship_Mode", "").strip()
        members = buckets.get(group, [])
        count = len(members)
        shipping = sum(r["shipping_milli"] for r in members)
        days = sum(r["delivery_days"] for r in members)
        actual = {
            "Order_Count": str(count),
            "Avg_Shipping_Cost": (
                fixed_string(divide(shipping, count * 1000), 2)
                if count
                else fixed_string(Decimal(0), 2)
            ),
            "Avg_Delivery_Days": (
                fixed_string(divide(days, count), 2)
                if count
                else fixed_string(Decimal(0), 2)
            ),
            "Total_Shipping_Cost": fixed_string(divide(shipping, 1000), 2),
        }
        decimals = {
            "Order_Count": 0,
            "Avg_Shipping_Cost": 2,
            "Avg_Delivery_Days": 2,
            "Total_Shipping_Cost": 2,
        }
        for field in fields:
            output.append(
                _reconciliation_entry(
                    SHIPPING_FILE, group, field, actual[field], rec.get(field, ""), decimals[field]
                )
            )
    return output


def _reconciliation_entry(source_file, group, field, actual, reference, places):
    reference_text = fixed_string(_decimal(reference), places)
    return {
        "source_file": source_file,
        "group": group,
        "field": field,
        "actual": actual,
        "reference": reference_text,
        "match": actual == reference_text,
    }


# --------------------------------------------------------------------------
# action candidates and optimization
# --------------------------------------------------------------------------
def build_candidates(effective_rows, policy):
    priorities = set(policy.get("shipping_priorities", []))
    saving_bp = int(policy.get("shipping_saving_bp", 0))
    threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_costs = policy.get("shipping_cost_by_market", {}) or {}
    discount_costs = policy.get("discount_cost_by_market", {}) or {}

    candidates = []
    for row in effective_rows:
        market = row["market"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = (row["shipping_milli"] * saving_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row["row_key"] + "|shipping_switch",
                        "row_key": row["row_key"],
                        "type": "shipping_switch",
                        "cost_units": int(shipping_costs.get(market, 0)),
                        "gain_milli": int(gain),
                    }
                )
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > threshold_ppm and contribution < 0:
            gain = (row["sales_milli"] * recovery_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row["row_key"] + "|discount_review",
                        "row_key": row["row_key"],
                        "type": "discount_review",
                        "cost_units": int(discount_costs.get(market, 0)),
                        "gain_milli": int(gain),
                    }
                )
    candidates.sort(key=lambda c: c["action_id"])
    return candidates


def optimize(candidates, budget):
    """Grouped knapsack: one action per row_key, exact lexicographic tie-break."""
    groups = {}
    for cand in candidates:
        groups.setdefault(cand["row_key"], []).append(cand)

    # state: cost -> (gain, tuple of action ids)
    states = {0: (0, ())}
    for row_key in sorted(groups):
        options = groups[row_key]
        new_states = {}
        for cost, (gain, ids) in states.items():
            for cand in [None] + options:
                extra_cost = 0 if cand is None else cand["cost_units"]
                extra_gain = 0 if cand is None else cand["gain_milli"]
                total_cost = cost + extra_cost
                if total_cost > budget:
                    continue
                total_gain = gain + extra_gain
                if cand is None:
                    new_ids = ids
                else:
                    new_ids = ids + (cand["action_id"],)
                best = new_states.get(total_cost)
                if best is None:
                    new_states[total_cost] = (total_gain, new_ids)
                    continue
                best_gain, best_ids = best
                if total_gain > best_gain:
                    new_states[total_cost] = (total_gain, new_ids)
                elif total_gain == best_gain:
                    if tuple(sorted(new_ids)) < tuple(sorted(best_ids)):
                        new_states[total_cost] = (total_gain, new_ids)
        states = new_states
        if not states:
            states = {0: (0, ())}
            break

    if not states:
        return [], 0, 0
    best_cost = None
    best_gain = None
    best_ids = None
    for cost in sorted(states):
        gain, ids = states[cost]
        if best_gain is None or gain > best_gain:
            best_gain, best_cost, best_ids = gain, cost, ids
    return list(best_ids), best_cost, best_gain


# --------------------------------------------------------------------------
# main
# --------------------------------------------------------------------------
def sha256_map(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path):
            continue
        hasher = hashlib.sha256()
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                hasher.update(chunk)
        digests[name] = hasher.hexdigest()
    return digests


def run(input_dir, output_dir):
    catalog = load_catalog(input_dir)
    rows = normalize_rows(input_dir, catalog)
    raw_events = load_events(os.path.join(input_dir, EVENTS_FILE))
    audit, effective = process_events(rows, raw_events)

    active_rows = [r for r in effective if r["active"]]
    aggregates = build_aggregates(rows, active_rows)
    summary = build_summary(rows, effective, audit)

    reconciliation = reconcile_segment(input_dir, rows) + reconcile_shipping(input_dir, rows)
    reconciliation.sort(key=lambda e: (e["source_file"], e["group"], e["field"]))

    with open(os.path.join(input_dir, POLICY_FILE), "r", encoding="utf-8") as handle:
        policy = json.load(handle)

    candidates = build_candidates(effective, policy)
    budget = int(policy.get("budget_units", 0))
    selected_ids, spent, gain = optimize(candidates, budget)
    by_id = {c["action_id"]: c for c in candidates}
    selected_actions = [by_id[aid] for aid in selected_ids]
    selected_actions.sort(key=lambda c: c["action_id"])

    current_contribution = sum(r["profit_milli"] - r["shipping_milli"] for r in active_rows)

    optimization = {
        "budget_units": budget,
        "spent_units": spent,
        "gain_milli": gain,
        "selected_action_ids": [c["action_id"] for c in selected_actions],
        "projected_contribution_milli": current_contribution + gain,
    }

    results = {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": rows,
        "event_audit": audit,
        "effective_rows": effective,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected_actions,
        "optimization": optimization,
        "input_sha256": sha256_map(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    target = os.path.join(output_dir, "results.json")
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, indent=2, sort_keys=True)
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
