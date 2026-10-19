#!/usr/bin/env python3
"""Global order audit, adjustment-event processing and budget optimisation.

Reads the fixed input bundle (five market order CSVs, product catalogue,
two reference summary tables, a priority codebook, a policy file and a JSONL
adjustment ledger) and writes a single deterministic ``results.json``.

Only the Python standard library is used.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import sys
from datetime import date
from decimal import Decimal, ROUND_HALF_UP
from fractions import Fraction

SCHEMA_VERSION = "107-v1"
MARKETS = ["Africa", "Asia_Pacific", "Europe", "LATAM", "USCA"]
ORDERS_SUFFIX = "_orders.csv"

EVENT_TYPES = ("cancel", "refund", "shipping_credit")

SEGMENT_REFERENCE_FIELDS = [
    ("Order_Count", 0),
    ("Total_Sales", 2),
    ("Total_Profit", 2),
    ("Profit_Margin", 2),
    ("Avg_Discount", 4),
]

SHIPPING_REFERENCE_FIELDS = [
    ("Order_Count", 0),
    ("Total_Shipping_Cost", 2),
    ("Avg_Shipping_Cost", 2),
    ("Avg_Delivery_Days", 2),
]


# ---------------------------------------------------------------------------
# numeric helpers
# ---------------------------------------------------------------------------


def scaled_int(value, scale):
    """Convert a decimal string to an exact integer scaled by ``scale``."""
    if value is None:
        return 0
    text = str(value).strip()
    if text == "":
        return 0
    dec = Decimal(text)
    return int((dec * scale).to_integral_value(rounding=ROUND_HALF_UP))


def to_milli(value):
    return scaled_int(value, 1000)


def to_ppm(value):
    return scaled_int(value, 1000000)


def round_half_up(value):
    """Round a Fraction to an int with ROUND_HALF_UP (half away from zero)."""
    num = value.numerator
    den = value.denominator
    sign = -1 if num < 0 else 1
    num = abs(num)
    quotient, remainder = divmod(num, den)
    if remainder * 2 >= den:
        quotient += 1
    return sign * quotient


def format_fraction(value, decimals):
    """Render a Fraction with a fixed number of decimals, ROUND_HALF_UP."""
    scaled = value * (10 ** decimals)
    rounded = round_half_up(scaled)
    negative = rounded < 0
    digits = str(abs(rounded))
    if decimals:
        digits = digits.rjust(decimals + 1, "0")
        digits = digits[:-decimals] + "." + digits[-decimals:]
    return ("-" if negative else "") + digits


def ratio_bp(numerator, denominator):
    """Integer basis points, or None when the denominator is zero."""
    if denominator == 0:
        return None
    return round_half_up(Fraction(numerator * 10000, denominator))


# ---------------------------------------------------------------------------
# input parsing
# ---------------------------------------------------------------------------


def read_table(path):
    """Read a CSV file, returning (header, records).

    Blank physical lines and records whose fields are all empty are skipped.
    """
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        header = None
        records = []
        for raw in reader:
            if not raw:
                continue
            if header is None:
                header = [cell.strip() for cell in raw]
                continue
            if all(cell is None or str(cell).strip() == "" for cell in raw):
                continue
            record = {}
            for index, name in enumerate(header):
                cell = raw[index] if index < len(raw) else ""
                record[name] = cell.strip() if isinstance(cell, str) else cell
            records.append(record)
    return header or [], records


def load_products(path):
    _header, records = read_table(path)
    catalogue = {}
    for record in records:
        product_id = record.get("Product ID", "")
        if product_id == "":
            continue
        catalogue[product_id] = record
    return catalogue


def read_jsonl(path):
    """Return (line_number, payload) for every non-empty JSONL record."""
    entries = []
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        for raw_line in handle:
            line = raw_line.strip()
            if not line:
                continue
            entries.append(json.loads(line))
    return entries


# ---------------------------------------------------------------------------
# normalisation
# ---------------------------------------------------------------------------


def normalise_rows(input_dir, catalogue):
    rows = []
    for market in MARKETS:
        filename = market + ORDERS_SUFFIX
        path = os.path.join(input_dir, filename)
        if not os.path.isfile(path):
            continue
        _header, records = read_table(path)
        for index, record in enumerate(records, start=1):
            product_id = record.get("Product ID", "")
            product = catalogue.get(product_id, {})
            order_date = record.get("Order Date", "")
            ship_date = record.get("Ship Date", "")
            rows.append(
                {
                    "row_key": "%s:%06d" % (filename, index),
                    "source_file": filename,
                    "source_row": index,
                    "order_id": record.get("Order ID", ""),
                    "product_id": product_id,
                    "market": market,
                    "category": product.get("Category", ""),
                    "sub_category": product.get("Sub-Category", ""),
                    "segment": record.get("Segment", ""),
                    "ship_mode": record.get("Ship Mode", ""),
                    "priority": record.get("Order Priority", ""),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": delivery_days(order_date, ship_date),
                    "quantity": scaled_int(record.get("Quantity", "0"), 1),
                    "sales_milli": to_milli(record.get("Sales", "0")),
                    "profit_milli": to_milli(record.get("Profit", "0")),
                    "shipping_milli": to_milli(record.get("Shipping Cost", "0")),
                    "discount_ppm": to_ppm(record.get("Discount", "0")),
                }
            )
    rows.sort(key=lambda item: item["row_key"])
    return rows


def delivery_days(order_date, ship_date):
    start = parse_iso_date(order_date)
    end = parse_iso_date(ship_date)
    if start is None or end is None:
        return 0
    return (end - start).days


def parse_iso_date(text):
    try:
        return date.fromisoformat(str(text).strip())
    except (ValueError, TypeError):
        return None


# ---------------------------------------------------------------------------
# adjustment ledger
# ---------------------------------------------------------------------------


def process_events(events, normalized_rows):
    """Apply the adjustment ledger, returning (audit, effective rows)."""
    state = {}
    for row in normalized_rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
            "cancelled": False,
        }

    # De-duplicate by event_id, keeping the first record in file order.
    seen = set()
    unique = []
    audit = []
    for line_number, event in enumerate(events, start=1):
        event_id = event.get("event_id")
        if event_id in seen:
            audit.append(
                {
                    "input_line": line_number,
                    "event_id": event_id,
                    "row_key": event.get("row_key"),
                    "status": "duplicate_event",
                }
            )
            continue
        seen.add(event_id)
        unique.append(event)
        audit.append(
            {
                "input_line": line_number,
                "event_id": event_id,
                "row_key": event.get("row_key"),
                "status": None,
            }
        )

    audit_by_id = {}
    for entry in audit:
        if entry["status"] is None:
            audit_by_id[entry["event_id"]] = entry

    ordered = sorted(
        unique,
        key=lambda event: (str(event.get("timestamp", "")), str(event.get("event_id", ""))),
    )

    for event in ordered:
        entry = audit_by_id[event.get("event_id")]
        row_key = event.get("row_key")
        event_type = event.get("type")
        amount = event.get("amount_milli")

        target = state.get(row_key)
        if target is None:
            entry["status"] = "unknown_row"
            continue
        if event_type not in EVENT_TYPES:
            entry["status"] = "unknown_type"
            continue
        if target["cancelled"]:
            entry["status"] = "order_cancelled"
            continue

        if not is_int(amount):
            entry["status"] = "invalid_amount"
            continue

        if event_type == "cancel":
            if amount != 0:
                entry["status"] = "invalid_amount"
                continue
            target["active"] = False
            target["cancelled"] = True
            target["sales_milli"] = 0
            target["profit_milli"] = 0
            target["shipping_milli"] = 0
            entry["status"] = "applied"
        elif event_type == "refund":
            if amount <= 0 or amount > target["sales_milli"]:
                entry["status"] = "invalid_amount"
                continue
            target["sales_milli"] -= amount
            target["profit_milli"] -= amount
            entry["status"] = "applied"
        else:  # shipping_credit
            if amount <= 0 or amount > target["shipping_milli"]:
                entry["status"] = "invalid_amount"
                continue
            target["shipping_milli"] -= amount
            entry["status"] = "applied"

    effective = []
    for row in normalized_rows:
        current = state[row["row_key"]]
        updated = dict(row)
        updated["active"] = bool(current["active"])
        updated["sales_milli"] = current["sales_milli"]
        updated["profit_milli"] = current["profit_milli"]
        updated["shipping_milli"] = current["shipping_milli"]
        effective.append(updated)

    return audit, effective


def is_int(value):
    return isinstance(value, int) and not isinstance(value, bool)


# ---------------------------------------------------------------------------
# metrics and aggregates
# ---------------------------------------------------------------------------


def metrics_for(rows):
    row_count = len(rows)
    quantity = 0
    sales = 0
    profit = 0
    shipping = 0
    high_discount = 0
    negative_contribution = 0
    for row in rows:
        quantity += row["quantity"]
        sales += row["sales_milli"]
        profit += row["profit_milli"]
        shipping += row["shipping_milli"]
        if row["discount_ppm"] > 200000:
            high_discount += 1
        if row["profit_milli"] < row["shipping_milli"]:
            negative_contribution += 1
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
        "high_discount_count": high_discount,
        "negative_contribution_count": negative_contribution,
    }


def build_aggregates(normalized_rows, effective_by_key):
    specs = [
        ("market", lambda row: (row["market"],)),
        ("market_category", lambda row: (row["market"], row["category"])),
        ("segment", lambda row: (row["segment"],)),
        ("ship_mode", lambda row: (row["ship_mode"],)),
        ("market_month", lambda row: (row["market"], row["order_date"][:7])),
    ]
    field_names = {
        "market": ["market"],
        "market_category": ["market", "category"],
        "segment": ["segment"],
        "ship_mode": ["ship_mode"],
        "market_month": ["market", "month"],
    }

    aggregates = {}
    for name, key_fn in specs:
        groups = {}
        for row in normalized_rows:
            key = key_fn(row)
            groups.setdefault(key, []).append(row["row_key"])
        entries = []
        for key in sorted(groups):
            active_rows = [
                effective_by_key[row_key]
                for row_key in groups[key]
                if effective_by_key[row_key]["active"]
            ]
            entry = {}
            for field, value in zip(field_names[name], key):
                entry[field] = value
            entry.update(metrics_for(active_rows))
            entries.append(entry)
        aggregates[name] = entries
    return aggregates


# ---------------------------------------------------------------------------
# reconciliation
# ---------------------------------------------------------------------------


def reconcile(input_dir, normalized_rows):
    results = []

    # --- segment_performance.csv -------------------------------------------
    segment_groups = {}
    for row in normalized_rows:
        segment_groups.setdefault(row["segment"], []).append(row)
    reference_path = os.path.join(input_dir, "segment_performance.csv")
    if os.path.isfile(reference_path):
        header, records = read_table(reference_path)
        for record in records:
            group = record.get("Segment", "")
            rows = segment_groups.get(group, [])
            count = len(rows)
            sales = sum(row["sales_milli"] for row in rows)
            profit = sum(row["profit_milli"] for row in rows)
            discount = sum(row["discount_ppm"] for row in rows)
            actuals = {
                "Order_Count": Fraction(count),
                "Total_Sales": Fraction(sales, 1000),
                "Total_Profit": Fraction(profit, 1000),
                "Profit_Margin": Fraction(profit, sales) * 100 if sales else Fraction(0),
                "Avg_Discount": Fraction(discount, count * 1000000) if count else Fraction(0),
            }
            results.extend(
                build_reconciliation_entries(
                    "segment_performance.csv",
                    group,
                    record,
                    actuals,
                    SEGMENT_REFERENCE_FIELDS,
                    header,
                )
            )

    # --- shipping_by_mode.csv ---------------------------------------------
    ship_groups = {}
    for row in normalized_rows:
        ship_groups.setdefault(row["ship_mode"], []).append(row)
    reference_path = os.path.join(input_dir, "shipping_by_mode.csv")
    if os.path.isfile(reference_path):
        header, records = read_table(reference_path)
        for record in records:
            group = record.get("Ship_Mode", "")
            rows = ship_groups.get(group, [])
            count = len(rows)
            shipping = sum(row["shipping_milli"] for row in rows)
            delivery = sum(row["delivery_days"] for row in rows)
            actuals = {
                "Order_Count": Fraction(count),
                "Total_Shipping_Cost": Fraction(shipping, 1000),
                "Avg_Shipping_Cost": Fraction(shipping, count * 1000) if count else Fraction(0),
                "Avg_Delivery_Days": Fraction(delivery, count) if count else Fraction(0),
            }
            results.extend(
                build_reconciliation_entries(
                    "shipping_by_mode.csv",
                    group,
                    record,
                    actuals,
                    SHIPPING_REFERENCE_FIELDS,
                    header,
                )
            )

    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


def build_reconciliation_entries(source_file, group, record, actuals, fields, header):
    entries = []
    for field, decimals in fields:
        if field not in header:
            continue
        actual_text = format_fraction(actuals[field], decimals)
        reference_raw = record.get(field, "")
        reference_value = parse_reference_value(reference_raw)
        reference_text = format_fraction(reference_value, decimals)
        entries.append(
            {
                "source_file": source_file,
                "group": group,
                "field": field,
                "actual": actual_text,
                "reference": reference_text,
                "match": actual_text == reference_text,
            }
        )
    return entries


def parse_reference_value(raw):
    if raw is None:
        return Fraction(0)
    text = str(raw).strip().replace(",", "")
    if text == "":
        return Fraction(0)
    return Fraction(Decimal(text))


# ---------------------------------------------------------------------------
# candidate actions and optimisation
# ---------------------------------------------------------------------------


def build_candidates(effective_rows, policy):
    priorities = set(policy.get("shipping_priorities", []) or [])
    saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold = int(policy.get("discount_threshold_ppm", 0))
    recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_cost = policy.get("shipping_cost_by_market", {}) or {}
    discount_cost = policy.get("discount_cost_by_market", {}) or {}

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]
        market = row["market"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = row["shipping_milli"] * saving_bp // 10000
            cost = int(shipping_cost.get(market, 0))
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row_key + "|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": cost,
                        "gain_milli": gain,
                    }
                )
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold and contribution < 0:
            gain = row["sales_milli"] * recovery_bp // 10000
            cost = int(discount_cost.get(market, 0))
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row_key + "|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": cost,
                        "gain_milli": gain,
                    }
                )
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimise(candidates, budget):
    """Exact 0/1 knapsack with lexicographic tie-breaking."""
    budget = max(0, int(budget))
    groups = {}
    for candidate in candidates:
        groups.setdefault(candidate["row_key"], []).append(candidate)

    max_cost = 0
    for options in groups.values():
        max_cost += max(option["cost_units"] for option in options)
    limit = min(budget, max_cost)

    # states: exact cost -> (gain, tuple of action_ids)
    states = {0: (0, ())}
    for row_key in sorted(groups):
        options = groups[row_key]
        updated = dict(states)
        for option in options:
            cost = option["cost_units"]
            gain = option["gain_milli"]
            action_id = option["action_id"]
            if cost > limit:
                continue
            for spent, (total_gain, ids) in states.items():
                new_spent = spent + cost
                if new_spent > limit:
                    continue
                new_gain = total_gain + gain
                new_ids = tuple(sorted(ids + (action_id,)))
                current = updated.get(new_spent)
                if (
                    current is None
                    or new_gain > current[0]
                    or (new_gain == current[0] and new_ids < current[1])
                ):
                    updated[new_spent] = (new_gain, new_ids)
        states = prune_states(updated)

    best_cost = 0
    best = states[0]
    for spent in sorted(states):
        gain, ids = states[spent]
        if (
            gain > best[0]
            or (gain == best[0] and spent < best_cost)
            or (gain == best[0] and spent == best_cost and ids < best[1])
        ):
            best = (gain, ids)
            best_cost = spent
    return best_cost, best[0], list(best[1])


def prune_states(states):
    """Drop states dominated by a cheaper one with at least the same gain."""
    kept = {}
    best_gain = None
    for spent in sorted(states):
        gain, ids = states[spent]
        if best_gain is None or gain > best_gain:
            kept[spent] = (gain, ids)
            best_gain = gain
    return kept


# ---------------------------------------------------------------------------
# hashing
# ---------------------------------------------------------------------------


def hash_inputs(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path):
            continue
        digest = hashlib.sha256()
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                digest.update(chunk)
        digests[name] = digest.hexdigest()
    return digests


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def build_results(input_dir):
    policy_path = os.path.join(input_dir, "policy.json")
    with open(policy_path, "r", encoding="utf-8-sig") as handle:
        policy = json.load(handle)

    catalogue = load_products(os.path.join(input_dir, "product_info.csv"))
    normalized_rows = normalise_rows(input_dir, catalogue)

    events = read_jsonl(os.path.join(input_dir, "adjustments.jsonl"))
    event_audit, effective_rows = process_events(events, normalized_rows)

    effective_by_key = {row["row_key"]: row for row in effective_rows}
    active_rows = [row for row in effective_rows if row["active"]]

    aggregates = build_aggregates(normalized_rows, effective_by_key)

    summary = metrics_for(active_rows)
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len({row["order_id"] for row in normalized_rows})
    summary["duplicate_order_id_extra_rows"] = (
        len(normalized_rows) - summary["unique_order_id_count"]
    )
    summary["cancelled_row_count"] = sum(1 for row in effective_rows if not row["active"])
    status_counts = {}
    for entry in event_audit:
        status = entry["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    summary["event_status_counts"] = status_counts

    reconciliation = reconcile(input_dir, normalized_rows)

    candidates = build_candidates(effective_rows, policy)
    spent, gain, selected_ids = optimise(candidates, policy.get("budget_units", 0))
    by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected_actions = [by_id[action_id] for action_id in selected_ids]

    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"] for row in active_rows
    )

    return {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized_rows,
        "event_audit": event_audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected_actions,
        "optimization": {
            "budget_units": int(policy.get("budget_units", 0)),
            "spent_units": spent,
            "gain_milli": gain,
            "selected_action_ids": selected_ids,
            "projected_contribution_milli": current_contribution + gain,
        },
        "input_sha256": hash_inputs(input_dir),
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args(argv)

    results = build_results(args.input)

    os.makedirs(args.output, exist_ok=True)
    target = os.path.join(args.output, "results.json")
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, sort_keys=True, indent=2)
        handle.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
