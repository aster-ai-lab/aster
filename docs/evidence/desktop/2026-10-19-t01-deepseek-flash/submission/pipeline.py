#!/usr/bin/env python3
"""Global order audit, event processing and budget optimization (107-V1).

Usage:
    python pipeline.py --input input --output output

Only the Python standard library is used.  All monetary values are handled as
integer thousandths of a currency unit (``_milli``), discounts as integer parts
per million (``_ppm``) and ratios as integer basis points (``_bp``).
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, localcontext

MARKETS = ("Africa", "Asia_Pacific", "Europe", "LATAM", "USCA")
MONEY_SCALE = 1000
PPM_SCALE = 1_000_000
EVENT_TYPES = ("cancel", "refund", "shipping_credit")
HIGH_DISCOUNT_PPM = 200_000

SEGMENT_FIELDS = (
    ("Order_Count", 0),
    ("Total_Sales", 2),
    ("Total_Profit", 2),
    ("Profit_Margin", 2),
    ("Avg_Discount", 4),
)
SHIPPING_FIELDS = (
    ("Order_Count", 0),
    ("Total_Shipping_Cost", 2),
    ("Avg_Shipping_Cost", 2),
    ("Avg_Delivery_Days", 2),
)


# ---------------------------------------------------------------------------
# Exact numeric helpers
# ---------------------------------------------------------------------------

def parse_scaled(text, scale):
    """Parse a decimal string into an exact integer scaled by ``scale``."""
    value = Decimal(str(text).strip())
    with localcontext() as ctx:
        ctx.prec = 80
        scaled = value * scale
    if scaled != scaled.to_integral_value():
        raise ValueError("value has more precision than supported: %r" % (text,))
    return int(scaled)


def round_half_up(num, den):
    """ROUND_HALF_UP of the exact rational num/den (ties away from zero)."""
    if den == 0:
        raise ZeroDivisionError("round_half_up with zero denominator")
    if den < 0:
        num, den = -num, -den
    negative = num < 0
    quotient, remainder = divmod(abs(num), den)
    if 2 * remainder >= den:
        quotient += 1
    return -quotient if negative else quotient


def format_scaled_int(value, places):
    """Format an integer scaled by 10**places as a fixed-point string."""
    negative = value < 0
    digits = str(abs(value))
    if places > 0:
        digits = digits.rjust(places + 1, "0")
        digits = digits[:-places] + "." + digits[-places:]
    return ("-" if negative else "") + digits


def format_fixed(num, den, places):
    """Exact num/den formatted with ``places`` decimals, ROUND_HALF_UP."""
    if den == 0:
        return format_scaled_int(0, places)
    if den < 0:
        num, den = -num, -den
    negative = num < 0
    scaled = abs(num) * (10 ** places)
    quotient, remainder = divmod(scaled, den)
    if 2 * remainder >= den:
        quotient += 1
    if negative:
        quotient = -quotient
    return format_scaled_int(quotient, places)


def ratio_bp(numerator, denominator):
    """Ratio in basis points with ROUND_HALF_UP, or None on zero denominator."""
    if denominator == 0:
        return None
    return round_half_up(numerator * 10000, denominator)


# ---------------------------------------------------------------------------
# Input loading
# ---------------------------------------------------------------------------

def read_csv_records(path):
    """Yield (record_number, dict) for data records, skipping blank lines."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        header = None
        record_number = 0
        for raw in reader:
            if not raw or all(cell.strip() == "" for cell in raw):
                continue
            if header is None:
                header = [cell.strip() for cell in raw]
                continue
            if len(raw) < len(header):
                raw = raw + [""] * (len(header) - len(raw))
            record = {}
            for index, name in enumerate(header):
                record[name] = raw[index].strip() if index < len(raw) else ""
            record_number += 1
            yield record_number, record


def load_catalog(input_dir):
    catalog = {}
    path = os.path.join(input_dir, "product_info.csv")
    for _, record in read_csv_records(path):
        catalog[record["Product ID"]] = (
            record.get("Category", ""),
            record.get("Sub-Category", ""),
        )
    return catalog


def load_orders(input_dir, catalog):
    rows = []
    for market in MARKETS:
        filename = market + "_orders.csv"
        path = os.path.join(input_dir, filename)
        for record_number, record in read_csv_records(path):
            product_id = record["Product ID"]
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date = record["Order Date"]
            ship_date = record["Ship Date"]
            delivery_days = (
                date.fromisoformat(ship_date) - date.fromisoformat(order_date)
            ).days
            rows.append(
                {
                    "row_key": "%s:%06d" % (filename, record_number),
                    "source_file": filename,
                    "source_row": record_number,
                    "order_id": record["Order ID"],
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": record["Segment"],
                    "ship_mode": record["Ship Mode"],
                    "priority": record["Order Priority"],
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": delivery_days,
                    "quantity": int(record["Quantity"]),
                    "sales_milli": parse_scaled(record["Sales"], MONEY_SCALE),
                    "profit_milli": parse_scaled(record["Profit"], MONEY_SCALE),
                    "shipping_milli": parse_scaled(
                        record["Shipping Cost"], MONEY_SCALE
                    ),
                    "discount_ppm": parse_scaled(record["Discount"], PPM_SCALE),
                }
            )
    rows.sort(key=lambda row: row["row_key"])
    return rows


def load_events(input_dir):
    """Return audit entries in input order and unique events for processing."""
    path = os.path.join(input_dir, "adjustments.jsonl")
    audit = []
    unique = []
    seen_ids = set()
    input_line = 0
    with open(path, "r", encoding="utf-8-sig") as handle:
        for raw_line in handle:
            if not raw_line.strip():
                continue
            input_line += 1
            payload = json.loads(raw_line)
            event_id = payload.get("event_id")
            entry = {
                "input_line": input_line,
                "event_id": event_id,
                "row_key": payload.get("row_key"),
                "status": None,
            }
            audit.append(entry)
            if event_id in seen_ids:
                entry["status"] = "duplicate_event"
            else:
                seen_ids.add(event_id)
                unique.append((payload, entry))
    return audit, unique


def apply_events(rows, unique_events):
    state = {}
    for row in rows:
        copy = dict(row)
        copy["active"] = True
        state[row["row_key"]] = copy

    ordered = sorted(
        unique_events, key=lambda item: (item[0]["timestamp"], item[0]["event_id"])
    )
    for payload, entry in ordered:
        row = state.get(payload.get("row_key"))
        event_type = payload.get("type")
        amount = payload.get("amount_milli")

        if row is None:
            entry["status"] = "unknown_row"
            continue
        if event_type not in EVENT_TYPES:
            entry["status"] = "unknown_type"
            continue
        if not row["active"]:
            entry["status"] = "order_cancelled"
            continue

        valid = type(amount) is int
        if valid:
            if event_type == "cancel":
                valid = amount == 0
            elif event_type == "refund":
                valid = amount > 0 and amount <= row["sales_milli"]
            else:  # shipping_credit
                valid = amount > 0 and amount <= row["shipping_milli"]

        if not valid:
            entry["status"] = "invalid_amount"
            continue

        if event_type == "cancel":
            row["active"] = False
            row["sales_milli"] = 0
            row["profit_milli"] = 0
            row["shipping_milli"] = 0
        elif event_type == "refund":
            row["sales_milli"] -= amount
            row["profit_milli"] -= amount
        else:
            row["shipping_milli"] -= amount
        entry["status"] = "applied"

    effective = [state[row["row_key"]] for row in rows]
    return effective


# ---------------------------------------------------------------------------
# Metrics and aggregates
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


def build_aggregates(normalized_rows, effective_rows):
    definitions = (
        ("market", ("market",)),
        ("market_category", ("market", "category")),
        ("segment", ("segment",)),
        ("ship_mode", ("ship_mode",)),
        ("market_month", ("market", "month")),
    )

    def group_key(row, fields):
        key = []
        for field in fields:
            if field == "month":
                key.append(row["order_date"][:7])
            else:
                key.append(row[field])
        return tuple(key)

    active_rows = [row for row in effective_rows if row["active"]]
    aggregates = {}
    for name, fields in definitions:
        keys = {group_key(row, fields) for row in normalized_rows}
        items = []
        for key in sorted(keys):
            members = [
                row for row in active_rows if group_key(row, fields) == key
            ]
            item = {}
            for field, value in zip(fields, key):
                item[field] = value
            item.update(compute_metrics(members))
            items.append(item)
        aggregates[name] = items
    return aggregates


# ---------------------------------------------------------------------------
# Reconciliation
# ---------------------------------------------------------------------------

def reference_decimal(raw, places):
    """Normalise a reference cell to a fixed-decimal string."""
    text = "" if raw is None else str(raw).strip()
    try:
        value = Decimal(text)
        with localcontext() as ctx:
            ctx.prec = 80
            scaled = (value * (10 ** places)).quantize(
                Decimal(1), rounding=ROUND_HALF_UP
            )
        return format_scaled_int(int(scaled), places)
    except Exception:
        return text


def segment_reference_values(rows, field, places):
    count = len(rows)
    sales = sum(row["sales_milli"] for row in rows)
    profit = sum(row["profit_milli"] for row in rows)
    discount_ppm = sum(row["discount_ppm"] for row in rows)
    if field == "Order_Count":
        return format_scaled_int(count, places)
    if field == "Total_Sales":
        return format_fixed(sales, MONEY_SCALE, places)
    if field == "Total_Profit":
        return format_fixed(profit, MONEY_SCALE, places)
    if field == "Profit_Margin":
        return format_fixed(profit * 100, sales, places)
    if field == "Avg_Discount":
        return format_fixed(discount_ppm, count * PPM_SCALE, places)
    raise KeyError(field)


def shipping_reference_values(rows, field, places):
    count = len(rows)
    shipping = sum(row["shipping_milli"] for row in rows)
    delivery_days = sum(row["delivery_days"] for row in rows)
    if field == "Order_Count":
        return format_scaled_int(count, places)
    if field == "Total_Shipping_Cost":
        return format_fixed(shipping, MONEY_SCALE, places)
    if field == "Avg_Shipping_Cost":
        return format_fixed(shipping, count * MONEY_SCALE, places)
    if field == "Avg_Delivery_Days":
        return format_fixed(delivery_days, count, places)
    raise KeyError(field)


def build_reconciliation(input_dir, normalized_rows):
    segment_rows = {}
    for row in normalized_rows:
        segment_rows.setdefault(row["segment"], []).append(row)
    ship_rows = {}
    for row in normalized_rows:
        ship_rows.setdefault(row["ship_mode"], []).append(row)

    tables = (
        (
            "segment_performance.csv",
            "Segment",
            SEGMENT_FIELDS,
            segment_rows,
            segment_reference_values,
        ),
        (
            "shipping_by_mode.csv",
            "Ship_Mode",
            SHIPPING_FIELDS,
            ship_rows,
            shipping_reference_values,
        ),
    )

    results = []
    for filename, group_column, fields, grouped, evaluator in tables:
        path = os.path.join(input_dir, filename)
        seen_groups = []
        seen = set()
        reference_by_group = {}
        for _, record in read_csv_records(path):
            group = record.get(group_column, "")
            if group in seen:
                continue
            seen.add(group)
            seen_groups.append(group)
            reference_by_group[group] = record
        for group in seen_groups:
            members = grouped.get(group, [])
            record = reference_by_group[group]
            for field, places in fields:
                actual = evaluator(members, field, places)
                reference = reference_decimal(record.get(field, ""), places)
                results.append(
                    {
                        "source_file": filename,
                        "group": group,
                        "field": field,
                        "actual": actual,
                        "reference": reference,
                        "match": actual == reference,
                    }
                )
    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


# ---------------------------------------------------------------------------
# Action candidates and exact budget optimization
# ---------------------------------------------------------------------------

def build_candidates(effective_rows, policy):
    priorities = set(policy.get("shipping_priorities", []))
    saving_bp = policy["shipping_saving_bp"]
    threshold_ppm = policy["discount_threshold_ppm"]
    recovery_bp = policy["discount_recovery_bp"]
    shipping_costs = policy["shipping_cost_by_market"]
    discount_costs = policy["discount_cost_by_market"]

    candidates = []
    for row in effective_rows:
        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = (row["shipping_milli"] * saving_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row["row_key"] + "|shipping_switch",
                        "row_key": row["row_key"],
                        "type": "shipping_switch",
                        "cost_units": shipping_costs[row["market"]],
                        "gain_milli": gain,
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
                        "cost_units": discount_costs[row["market"]],
                        "gain_milli": gain,
                    }
                )
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimize(candidates, budget_units, total_contribution):
    by_row = {}
    for candidate in candidates:
        by_row.setdefault(candidate["row_key"], []).append(candidate)

    upper_bound = sum(
        max(candidate["cost_units"] for candidate in options)
        for options in by_row.values()
    )
    cap = min(max(budget_units, 0), upper_bound)

    # dp[cost] = (gain, tuple of action ids); one choice per row_key.
    dp = {0: (0, ())}
    for row_key in sorted(by_row):
        options = by_row[row_key]
        updated = dict(dp)
        for cost, (gain, action_ids) in dp.items():
            for candidate in options:
                new_cost = cost + candidate["cost_units"]
                if new_cost > cap:
                    continue
                new_gain = gain + candidate["gain_milli"]
                new_ids = action_ids + (candidate["action_id"],)
                current = updated.get(new_cost)
                if (
                    current is None
                    or new_gain > current[0]
                    or (new_gain == current[0] and new_ids < current[1])
                ):
                    updated[new_cost] = (new_gain, new_ids)
        dp = updated

    best_gain = 0
    best_ids = ()
    best_cost = 0
    for cost in sorted(dp):
        gain, action_ids = dp[cost]
        if gain > best_gain:
            best_gain = gain
            best_ids = action_ids
            best_cost = cost

    by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected = [by_id[action_id] for action_id in best_ids]
    optimization = {
        "budget_units": budget_units,
        "spent_units": best_cost,
        "gain_milli": best_gain,
        "selected_action_ids": list(best_ids),
        "projected_contribution_milli": total_contribution + best_gain,
    }
    return selected, optimization


# ---------------------------------------------------------------------------
# Pipeline
# ---------------------------------------------------------------------------

def input_digests(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path):
            continue
        with open(path, "rb") as handle:
            digests[name] = hashlib.sha256(handle.read()).hexdigest()
    return digests


def run(input_dir, output_dir):
    catalog = load_catalog(input_dir)
    normalized_rows = load_orders(input_dir, catalog)
    audit, unique_events = load_events(input_dir)
    effective_rows = apply_events(normalized_rows, unique_events)

    for row in effective_rows:
        row.setdefault("active", True)
    effective_rows.sort(key=lambda row: row["row_key"])

    active_rows = [row for row in effective_rows if row["active"]]
    summary = compute_metrics(active_rows)
    summary["input_row_count"] = len(normalized_rows)
    unique_orders = len({row["order_id"] for row in normalized_rows})
    summary["unique_order_id_count"] = unique_orders
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - unique_orders
    summary["cancelled_row_count"] = sum(
        1 for row in effective_rows if not row["active"]
    )
    status_counts = {}
    for entry in audit:
        status_counts[entry["status"]] = status_counts.get(entry["status"], 0) + 1
    summary["event_status_counts"] = status_counts

    aggregates = build_aggregates(normalized_rows, effective_rows)
    reconciliation = build_reconciliation(input_dir, normalized_rows)

    with open(os.path.join(input_dir, "policy.json"), "r", encoding="utf-8") as handle:
        policy = json.load(handle)

    candidates = build_candidates(effective_rows, policy)
    total_contribution = summary["contribution_milli"]
    selected, optimization = optimize(
        candidates, policy["budget_units"], total_contribution
    )

    result = {
        "schema_version": "107-v1",
        "normalized_rows": normalized_rows,
        "event_audit": audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected,
        "optimization": optimization,
        "input_sha256": input_digests(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    payload = json.dumps(
        result, ensure_ascii=False, sort_keys=True, indent=2, allow_nan=False
    )
    with open(os.path.join(output_dir, "results.json"), "wb") as handle:
        handle.write(payload.encode("utf-8") + b"\n")
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Audit orders, apply events and optimize actions."
    )
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args(argv)
    run(os.path.abspath(args.input), os.path.abspath(args.output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
