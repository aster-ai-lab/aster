#!/usr/bin/env python3
"""Global order audit, event processing, metrics and budget optimisation.

Only the Python standard library is used.  The program reads an input
directory and writes <output>/results.json deterministically.
"""

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, localcontext

SCHEMA_VERSION = "107-v1"
ORDERS_SUFFIX = "_orders.csv"
EVENT_TYPES = {"cancel", "refund", "shipping_credit"}
HIGH_DISCOUNT_PPM = 200000


# ---------------------------------------------------------------------------
# Numeric helpers
# ---------------------------------------------------------------------------

def to_scaled_int(raw, scale):
    """Convert an exact decimal string to an integer scaled by ``scale``.

    Inputs carry at most three (money) or six (discount) decimal places, so
    the multiplication is exact.  If a value somehow carries more decimals,
    round half up at the target precision rather than through binary floats.
    """
    value = Decimal(str(raw).strip())
    with localcontext() as ctx:
        ctx.prec = 60
        scaled = value * Decimal(scale)
        if scaled == scaled.to_integral_value():
            return int(scaled)
        return int(scaled.quantize(Decimal(1), rounding=ROUND_HALF_UP))


def to_milli(raw):
    return to_scaled_int(raw, 1000)


def to_ppm(raw):
    return to_scaled_int(raw, 1000000)


def ratio_bp(numerator, denominator):
    """ROUND_HALF_UP(numerator / denominator * 10000), None for zero divisor."""
    if denominator == 0:
        return None
    with localcontext() as ctx:
        ctx.prec = 60
        value = Decimal(numerator) / Decimal(denominator) * Decimal(10000)
        return int(value.quantize(Decimal(1), rounding=ROUND_HALF_UP))


def format_fixed(value, places):
    """Format a Decimal with exactly ``places`` decimals, half up."""
    with localcontext() as ctx:
        ctx.prec = 60
        quantum = Decimal(1).scaleb(-places)
        rounded = value.quantize(quantum, rounding=ROUND_HALF_UP)
    return f"{rounded:.{places}f}"


def days_between(order_date, ship_date):
    return (date.fromisoformat(ship_date) - date.fromisoformat(order_date)).days


# ---------------------------------------------------------------------------
# Input loading
# ---------------------------------------------------------------------------

def load_products(input_dir):
    products = {}
    path = os.path.join(input_dir, "product_info.csv")
    with open(path, newline="", encoding="utf-8-sig") as handle:
        reader = csv.reader(handle)
        header = None
        for record in reader:
            if not record or all(cell.strip() == "" for cell in record):
                continue
            if header is None:
                header = [cell.strip() for cell in record]
                index = {name: i for i, name in enumerate(header)}
                continue

            def cell(name, default=""):
                i = index.get(name)
                if i is None or i >= len(record):
                    return default
                return record[i].strip()

            product_id = cell("Product ID")
            products[product_id] = {
                "category": cell("Category"),
                "sub_category": cell("Sub-Category"),
            }
    return products


def load_orders(input_dir, products):
    rows = []
    names = sorted(
        name
        for name in os.listdir(input_dir)
        if name.endswith(ORDERS_SUFFIX)
        and os.path.isfile(os.path.join(input_dir, name))
    )
    for name in names:
        market = name[: -len(ORDERS_SUFFIX)]
        path = os.path.join(input_dir, name)
        with open(path, newline="", encoding="utf-8-sig") as handle:
            reader = csv.reader(handle)
            header = None
            index = {}
            source_row = 0
            for record in reader:
                if not record or all(cell.strip() == "" for cell in record):
                    continue
                if header is None:
                    header = [cell.strip() for cell in record]
                    index = {column: i for i, column in enumerate(header)}
                    continue

                def cell(column):
                    i = index[column]
                    return record[i].strip() if i < len(record) else ""

                source_row += 1
                product_id = cell("Product ID")
                info = products.get(product_id, {"category": "", "sub_category": ""})
                order_date = cell("Order Date")
                ship_date = cell("Ship Date")
                rows.append(
                    {
                        "row_key": f"{name}:{source_row:06d}",
                        "source_file": name,
                        "source_row": source_row,
                        "order_id": cell("Order ID"),
                        "product_id": product_id,
                        "market": market,
                        "category": info["category"],
                        "sub_category": info["sub_category"],
                        "segment": cell("Segment"),
                        "ship_mode": cell("Ship Mode"),
                        "priority": cell("Order Priority"),
                        "order_date": order_date,
                        "ship_date": ship_date,
                        "delivery_days": days_between(order_date, ship_date),
                        "quantity": int(cell("Quantity")),
                        "sales_milli": to_milli(cell("Sales")),
                        "profit_milli": to_milli(cell("Profit")),
                        "shipping_milli": to_milli(cell("Shipping Cost")),
                        "discount_ppm": to_ppm(cell("Discount")),
                    }
                )
    rows.sort(key=lambda row: row["row_key"])
    return rows


def load_events(input_dir):
    """Return [(non_empty_line_number, event_object)] in file order."""
    records = []
    path = os.path.join(input_dir, "adjustments.jsonl")
    with open(path, encoding="utf-8-sig") as handle:
        line_number = 0
        for line in handle:
            if line.strip() == "":
                continue
            line_number += 1
            records.append((line_number, json.loads(line)))
    return records


# ---------------------------------------------------------------------------
# Event ledger
# ---------------------------------------------------------------------------

def is_json_integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def process_events(normalized_rows, records):
    """Apply deduplicated events in (timestamp, event_id) order.

    Returns (audit, effective_rows).
    """
    effective = {}
    for row in normalized_rows:
        copy = dict(row)
        copy["active"] = True
        effective[row["row_key"]] = copy
    cancelled = {row["row_key"]: False for row in normalized_rows}

    audit = [None] * len(records)
    seen_ids = set()
    retained = []  # (audit_index, line_number, event)

    for position, (line_number, event) in enumerate(records):
        event_id = event.get("event_id")
        row_key = event.get("row_key")
        entry = {
            "input_line": line_number,
            "event_id": event_id,
            "row_key": row_key,
            "status": None,
        }
        audit[position] = entry
        if event_id in seen_ids:
            entry["status"] = "duplicate_event"
        else:
            seen_ids.add(event_id)
            retained.append((position, event))

    retained.sort(
        key=lambda item: (
            item[1].get("timestamp", ""),
            str(item[1].get("event_id")),
        )
    )

    for position, event in retained:
        status = apply_event(event, effective, cancelled)
        audit[position]["status"] = status

    effective_rows = [effective[row["row_key"]] for row in normalized_rows]
    return audit, effective_rows


def apply_event(event, effective, cancelled):
    row_key = event.get("row_key")
    if row_key not in effective:
        return "unknown_row"

    event_type = event.get("type")
    if event_type not in EVENT_TYPES:
        return "unknown_type"

    if cancelled.get(row_key, False):
        return "order_cancelled"

    amount = event.get("amount_milli")
    valid_integer = is_json_integer(amount)
    row = effective[row_key]

    if event_type == "cancel":
        if not (valid_integer and amount == 0):
            return "invalid_amount"
        row["active"] = False
        cancelled[row_key] = True
        row["sales_milli"] = 0
        row["profit_milli"] = 0
        row["shipping_milli"] = 0
        return "applied"

    if event_type == "refund":
        if not (valid_integer and amount > 0 and amount <= row["sales_milli"]):
            return "invalid_amount"
        row["sales_milli"] -= amount
        row["profit_milli"] -= amount
        return "applied"

    # shipping_credit
    if not (valid_integer and amount > 0 and amount <= row["shipping_milli"]):
        return "invalid_amount"
    row["shipping_milli"] -= amount
    return "applied"


# ---------------------------------------------------------------------------
# Metrics and aggregates
# ---------------------------------------------------------------------------

def compute_metrics(rows):
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
        if row["discount_ppm"] > HIGH_DISCOUNT_PPM:
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


def build_aggregates(normalized_rows, effective_rows):
    def month_of(row):
        return row["order_date"][:7]

    specifications = [
        ("market", ["market"], lambda row: (row["market"],)),
        ("market_category", ["market", "category"],
         lambda row: (row["market"], row["category"])),
        ("segment", ["segment"], lambda row: (row["segment"],)),
        ("ship_mode", ["ship_mode"], lambda row: (row["ship_mode"],)),
        ("market_month", ["market", "month"],
         lambda row: (row["market"], month_of(row))),
    ]

    active_rows = [row for row in effective_rows if row["active"]]

    aggregates = {}
    for name, field_names, key_function in specifications:
        all_keys = set()
        for row in normalized_rows:
            all_keys.add(key_function(row))

        buckets = {}
        for row in active_rows:
            buckets.setdefault(key_function(row), []).append(row)

        items = []
        for key in sorted(all_keys):
            item = dict(zip(field_names, key))
            item.update(compute_metrics(buckets.get(key, [])))
            items.append(item)
        aggregates[name] = items

    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    active_rows = [row for row in effective_rows if row["active"]]
    summary = compute_metrics(active_rows)

    order_ids = {row["order_id"] for row in normalized_rows}
    status_counts = {}
    for entry in audit:
        status = entry["status"]
        status_counts[status] = status_counts.get(status, 0) + 1

    summary.update(
        {
            "input_row_count": len(normalized_rows),
            "unique_order_id_count": len(order_ids),
            "duplicate_order_id_extra_rows": len(normalized_rows) - len(order_ids),
            "cancelled_row_count": sum(
                1 for row in effective_rows if not row["active"]
            ),
            "event_status_counts": status_counts,
        }
    )
    return summary


# ---------------------------------------------------------------------------
# Reconciliation of the reference reports
# ---------------------------------------------------------------------------

SEGMENT_FIELDS = [
    "Order_Count",
    "Total_Sales",
    "Total_Profit",
    "Profit_Margin",
    "Avg_Discount",
]

SHIP_MODE_FIELDS = [
    "Order_Count",
    "Total_Shipping_Cost",
    "Avg_Shipping_Cost",
    "Avg_Delivery_Days",
]


def load_reference(input_dir, filename):
    path = os.path.join(input_dir, filename)
    with open(path, newline="", encoding="utf-8-sig") as handle:
        reader = csv.DictReader(handle)
        header = [name.strip() for name in (reader.fieldnames or [])]
        rows = []
        for raw in reader:
            rows.append(
                {(key.strip() if key else key): (value.strip() if isinstance(value, str) else value)
                 for key, value in raw.items()}
            )
    return header, rows


def reference_string(raw, places):
    if raw is None or raw == "":
        value = Decimal(0)
    else:
        value = Decimal(str(raw))
    return format_fixed(value, places)


def build_reconciliation(input_dir, normalized_rows):
    results = []

    # segment_performance.csv -------------------------------------------------
    header, reference_rows = load_reference(input_dir, "segment_performance.csv")
    stats = {}
    for row in normalized_rows:
        bucket = stats.setdefault(row["segment"], [0, 0, 0, 0])
        bucket[0] += 1
        bucket[1] += row["sales_milli"]
        bucket[2] += row["profit_milli"]
        bucket[3] += row["discount_ppm"]

    fields = [field for field in SEGMENT_FIELDS if field in header]
    for reference_row in reference_rows:
        group = reference_row.get("Segment", "")
        count, sales, profit, discount_sum = stats.get(group, [0, 0, 0, 0])
        actual = {
            "Order_Count": format_fixed(Decimal(count), 0),
            "Total_Sales": format_fixed(Decimal(sales) / Decimal(1000), 2),
            "Total_Profit": format_fixed(Decimal(profit) / Decimal(1000), 2),
            "Profit_Margin": (
                format_fixed(
                    Decimal(profit) / Decimal(sales) * Decimal(100), 2
                )
                if sales != 0
                else format_fixed(Decimal(0), 2)
            ),
            "Avg_Discount": (
                format_fixed(
                    Decimal(discount_sum) / Decimal(count) / Decimal(1000000), 4
                )
                if count != 0
                else format_fixed(Decimal(0), 4)
            ),
        }
        places = {
            "Order_Count": 0,
            "Total_Sales": 2,
            "Total_Profit": 2,
            "Profit_Margin": 2,
            "Avg_Discount": 4,
        }
        for field in fields:
            actual_value = actual[field]
            reference_value = reference_string(reference_row.get(field), places[field])
            results.append(
                {
                    "source_file": "segment_performance.csv",
                    "group": group,
                    "field": field,
                    "actual": actual_value,
                    "reference": reference_value,
                    "match": actual_value == reference_value,
                }
            )

    # shipping_by_mode.csv ----------------------------------------------------
    header, reference_rows = load_reference(input_dir, "shipping_by_mode.csv")
    stats = {}
    for row in normalized_rows:
        bucket = stats.setdefault(row["ship_mode"], [0, 0, 0])
        bucket[0] += 1
        bucket[1] += row["shipping_milli"]
        bucket[2] += row["delivery_days"]

    fields = [field for field in SHIP_MODE_FIELDS if field in header]
    for reference_row in reference_rows:
        group = reference_row.get("Ship_Mode", "")
        count, shipping, delivery_days = stats.get(group, [0, 0, 0])
        actual = {
            "Order_Count": format_fixed(Decimal(count), 0),
            "Total_Shipping_Cost": format_fixed(Decimal(shipping) / Decimal(1000), 2),
            "Avg_Shipping_Cost": (
                format_fixed(
                    Decimal(shipping) / Decimal(count) / Decimal(1000), 2
                )
                if count != 0
                else format_fixed(Decimal(0), 2)
            ),
            "Avg_Delivery_Days": (
                format_fixed(Decimal(delivery_days) / Decimal(count), 2)
                if count != 0
                else format_fixed(Decimal(0), 2)
            ),
        }
        places = {
            "Order_Count": 0,
            "Total_Shipping_Cost": 2,
            "Avg_Shipping_Cost": 2,
            "Avg_Delivery_Days": 2,
        }
        for field in fields:
            actual_value = actual[field]
            reference_value = reference_string(reference_row.get(field), places[field])
            results.append(
                {
                    "source_file": "shipping_by_mode.csv",
                    "group": group,
                    "field": field,
                    "actual": actual_value,
                    "reference": reference_value,
                    "match": actual_value == reference_value,
                }
            )

    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


# ---------------------------------------------------------------------------
# Action candidates and exact budget optimisation
# ---------------------------------------------------------------------------

def build_candidates(effective_rows, policy):
    priorities = set(policy["shipping_priorities"])
    saving_bp = int(policy["shipping_saving_bp"])
    threshold_ppm = int(policy["discount_threshold_ppm"])
    recovery_bp = int(policy["discount_recovery_bp"])
    shipping_costs = policy["shipping_cost_by_market"]
    discount_costs = policy["discount_cost_by_market"]

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]

        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = (row["shipping_milli"] * saving_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": f"{row_key}|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": int(shipping_costs[row["market"]]),
                        "gain_milli": gain,
                    }
                )

        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > threshold_ppm and contribution < 0:
            gain = (row["sales_milli"] * recovery_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": f"{row_key}|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": int(discount_costs[row["market"]]),
                        "gain_milli": gain,
                    }
                )

    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimise(candidates, budget_units):
    """Exact multiple-choice knapsack.

    Objective (lexicographic): maximise gain, minimise cost, then choose the
    lexicographically smallest ascending action_id list.  For a fixed
    (cost, gain) the lexicographic preference is encoded as a bit mask where
    bit n-1-i marks candidate i (candidates sorted by action_id): the set
    containing the earliest differing action_id has the larger mask and the
    lexicographically smaller list.
    """
    total_candidates = len(candidates)
    bit_for = {
        candidate["action_id"]: 1 << (total_candidates - 1 - index)
        for index, candidate in enumerate(candidates)
    }

    groups = {}
    for candidate in candidates:
        groups.setdefault(candidate["row_key"], []).append(candidate)

    if budget_units < 0:
        budget_units = 0
    maximum_spend = sum(
        max(candidate["cost_units"] for candidate in group)
        for group in groups.values()
    )
    cap = min(budget_units, maximum_spend)

    # cost -> (gain, mask); only the Pareto frontier is retained.
    states = {0: (0, 0)}
    for row_key in sorted(groups):
        choices = [(0, 0, 0)]  # skip this row
        for candidate in groups[row_key]:
            choices.append(
                (
                    candidate["cost_units"],
                    candidate["gain_milli"],
                    bit_for[candidate["action_id"]],
                )
            )

        updated = {}
        for cost, (gain, mask) in states.items():
            for extra_cost, extra_gain, extra_mask in choices:
                new_cost = cost + extra_cost
                if new_cost > cap:
                    continue
                new_gain = gain + extra_gain
                new_mask = mask | extra_mask
                current = updated.get(new_cost)
                if (
                    current is None
                    or new_gain > current[0]
                    or (new_gain == current[0] and new_mask > current[1])
                ):
                    updated[new_cost] = (new_gain, new_mask)

        # A state with higher cost and no higher gain is dominated.
        states = {}
        best_gain = -1
        for cost in sorted(updated):
            gain, mask = updated[cost]
            if gain > best_gain:
                states[cost] = (gain, mask)
                best_gain = gain

    if not states:
        states = {0: (0, 0)}

    best_cost, (best_gain, best_mask) = max(
        states.items(), key=lambda item: (item[1][0], -item[0], item[1][1])
    )

    selected = [
        candidate
        for index, candidate in enumerate(candidates)
        if best_mask & (1 << (total_candidates - 1 - index))
    ]
    selected.sort(key=lambda item: item["action_id"])

    spent = sum(item["cost_units"] for item in selected)
    gain = sum(item["gain_milli"] for item in selected)

    return selected, spent, gain


# ---------------------------------------------------------------------------
# Provenance and output
# ---------------------------------------------------------------------------

def compute_input_sha256(input_dir):
    digests = {}
    for entry in sorted(os.scandir(input_dir), key=lambda item: item.name):
        if not entry.is_file(follow_symlinks=False):
            continue
        digest = hashlib.sha256()
        with open(entry.path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        digests[entry.name] = digest.hexdigest()
    return digests


def build_result(input_dir):
    products = load_products(input_dir)
    normalized_rows = load_orders(input_dir, products)
    event_records = load_events(input_dir)

    audit, effective_rows = process_events(normalized_rows, event_records)

    aggregates = build_aggregates(normalized_rows, effective_rows)
    reconciliation = build_reconciliation(input_dir, normalized_rows)
    summary = build_summary(normalized_rows, effective_rows, audit)

    with open(os.path.join(input_dir, "policy.json"), encoding="utf-8-sig") as handle:
        policy = json.load(handle)

    action_candidates = build_candidates(effective_rows, policy)
    selected_actions, spent_units, gain_milli = optimise(
        action_candidates, int(policy["budget_units"])
    )

    optimization = {
        "budget_units": int(policy["budget_units"]),
        "spent_units": spent_units,
        "gain_milli": gain_milli,
        "selected_action_ids": [item["action_id"] for item in selected_actions],
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
        "action_candidates": action_candidates,
        "selected_actions": selected_actions,
        "optimization": optimization,
        "input_sha256": compute_input_sha256(input_dir),
    }


def main():
    parser = argparse.ArgumentParser(
        description="Global order audit, event processing and budget optimisation"
    )
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    arguments = parser.parse_args()

    result = build_result(arguments.input)

    os.makedirs(arguments.output, exist_ok=True)
    output_path = os.path.join(arguments.output, "results.json")
    with open(output_path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(result, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")


if __name__ == "__main__":
    main()
