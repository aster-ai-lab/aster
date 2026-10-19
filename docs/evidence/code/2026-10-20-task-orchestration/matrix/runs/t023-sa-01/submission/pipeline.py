#!/usr/bin/env python3
"""107-V1 order audit, adjustment ledger, aggregates, reconciliation and
budgeted action selection.

Standard library only.  Run with:
    python pipeline.py --input input --output output
"""

import argparse
import csv
import hashlib
import json
import os
from collections import Counter, defaultdict
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, getcontext

getcontext().prec = 80

EVENT_TYPES = ("cancel", "refund", "shipping_credit")
SCHEMA_VERSION = "107-v1"

METRIC_KEYS = (
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


# ---------------------------------------------------------------------------
# Small numeric / formatting helpers
# ---------------------------------------------------------------------------

def scaled_int(text, scale):
    """Convert a decimal string to an exact integer in units of 1/scale."""
    value = Decimal(str(text).strip()) * Decimal(scale)
    # Inputs are guaranteed to have at most the supported number of decimals.
    # The rounding fallback only guards against malformed extra precision.
    return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def ratio_bp(numerator, denominator):
    """ROUND_HALF_UP(numerator / denominator * 10000); None for zero denom."""
    if denominator == 0:
        return None
    negative = (numerator < 0) != (denominator < 0)
    n = abs(numerator)
    d = abs(denominator)
    q, r = divmod(n * 10000, d)
    if r * 2 >= d:
        q += 1
    return -q if negative else q


def format_fixed(value, places):
    """Format a Decimal with exactly `places` decimals using ROUND_HALF_UP."""
    quantum = Decimal(1).scaleb(-places)
    rounded = value.quantize(quantum, rounding=ROUND_HALF_UP)
    if rounded == 0:
        rounded = abs(rounded)  # avoid "-0.00"
    return format(rounded, "." + str(places) + "f")


def format_reference(raw, places):
    """Format a reference value to the same fixed precision, if numeric."""
    text = ("" if raw is None else str(raw)).strip()
    try:
        return format_fixed(Decimal(text), places), True
    except Exception:
        return text, False


# ---------------------------------------------------------------------------
# CSV loading
# ---------------------------------------------------------------------------

def read_csv_records(path):
    """Read a CSV, skipping physical blank lines and all-empty records."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        try:
            header = next(reader)
        except StopIteration:
            return [], []
        header = [(name or "").strip() for name in header]
        records = []
        for raw in reader:
            if not raw:
                continue
            if all((cell or "").strip() == "" for cell in raw):
                continue
            record = {}
            for index, name in enumerate(header):
                record[name] = raw[index] if index < len(raw) else ""
            records.append(record)
    return header, records


def load_products(path):
    products = {}
    if not os.path.exists(path):
        return products
    _, records = read_csv_records(path)
    for record in records:
        product_id = (record.get("Product ID") or "").strip()
        if not product_id or product_id in products:
            continue
        products[product_id] = (
            (record.get("Category") or "").strip(),
            (record.get("Sub-Category") or "").strip(),
        )
    return products


def order_files(input_dir):
    names = []
    for name in os.listdir(input_dir):
        if name.endswith("_orders.csv") and os.path.isfile(
            os.path.join(input_dir, name)
        ):
            names.append(name)
    return sorted(names)


def load_normalized_rows(input_dir, products):
    rows = []
    for file_name in order_files(input_dir):
        market = file_name[: -len("_orders.csv")]
        _, records = read_csv_records(os.path.join(input_dir, file_name))
        for sequence, record in enumerate(records, 1):
            product_id = (record.get("Product ID") or "").strip()
            category, sub_category = products.get(product_id, ("", ""))
            order_date = (record.get("Order Date") or "").strip()
            ship_date = (record.get("Ship Date") or "").strip()
            delivery_days = (
                date.fromisoformat(ship_date) - date.fromisoformat(order_date)
            ).days
            rows.append(
                {
                    "row_key": "%s:%06d" % (file_name, sequence),
                    "source_file": file_name,
                    "source_row": sequence,
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
                    "quantity": int((record.get("Quantity") or "0").strip()),
                    "sales_milli": scaled_int(record.get("Sales") or "0", 1000),
                    "profit_milli": scaled_int(record.get("Profit") or "0", 1000),
                    "shipping_milli": scaled_int(
                        record.get("Shipping Cost") or "0", 1000
                    ),
                    "discount_ppm": scaled_int(
                        record.get("Discount") or "0", 1000000
                    ),
                }
            )
    rows.sort(key=lambda row: row["row_key"])
    return rows


# ---------------------------------------------------------------------------
# Adjustment event ledger
# ---------------------------------------------------------------------------

def load_events(path):
    events = []
    if not os.path.exists(path):
        return events
    input_line = 0
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            input_line += 1
            event = json.loads(line)
            event["input_line"] = input_line
            events.append(event)
    return events


def is_integer(value):
    return isinstance(value, int) and not isinstance(value, bool)


def process_events(normalized_rows, events):
    seen = set()
    unique = []
    for event in events:
        event_id = event.get("event_id")
        if event_id in seen:
            event["_duplicate"] = True
        else:
            seen.add(event_id)
            event["_duplicate"] = False
            unique.append(event)

    states = {}
    for row in normalized_rows:
        states[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    ordered = sorted(
        unique,
        key=lambda event: (
            event.get("timestamp", ""),
            event.get("event_id", ""),
        ),
    )
    for event in ordered:
        row_key = event.get("row_key")
        state = states.get(row_key)
        event_type = event.get("type")
        if state is None:
            status = "unknown_row"
        elif event_type not in EVENT_TYPES:
            status = "unknown_type"
        elif not state["active"]:
            status = "order_cancelled"
        else:
            amount = event.get("amount_milli")
            valid = False
            if event_type == "cancel":
                valid = is_integer(amount) and amount == 0
                if valid:
                    state["active"] = False
                    state["sales_milli"] = 0
                    state["profit_milli"] = 0
                    state["shipping_milli"] = 0
            elif event_type == "refund":
                valid = (
                    is_integer(amount)
                    and amount > 0
                    and amount <= state["sales_milli"]
                )
                if valid:
                    state["sales_milli"] -= amount
                    state["profit_milli"] -= amount
            else:  # shipping_credit
                valid = (
                    is_integer(amount)
                    and amount > 0
                    and amount <= state["shipping_milli"]
                )
                if valid:
                    state["shipping_milli"] -= amount
            status = "applied" if valid else "invalid_amount"
        event["_status"] = status

    audit = []
    for event in events:
        status = (
            "duplicate_event" if event.get("_duplicate") else event.get("_status")
        )
        audit.append(
            {
                "input_line": event["input_line"],
                "event_id": event.get("event_id"),
                "row_key": event.get("row_key"),
                "status": status,
            }
        )

    effective = []
    for row in normalized_rows:
        state = states[row["row_key"]]
        updated = dict(row)
        updated["active"] = state["active"]
        updated["sales_milli"] = state["sales_milli"]
        updated["profit_milli"] = state["profit_milli"]
        updated["shipping_milli"] = state["shipping_milli"]
        effective.append(updated)

    return audit, effective


# ---------------------------------------------------------------------------
# Aggregates
# ---------------------------------------------------------------------------

GROUP_DEFINITIONS = (
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


def new_accumulator():
    return {
        "row_count": 0,
        "quantity": 0,
        "sales_milli": 0,
        "profit_milli": 0,
        "shipping_milli": 0,
        "high_discount_count": 0,
        "negative_contribution_count": 0,
    }


def add_row(accumulator, row):
    accumulator["row_count"] += 1
    accumulator["quantity"] += row["quantity"]
    accumulator["sales_milli"] += row["sales_milli"]
    accumulator["profit_milli"] += row["profit_milli"]
    accumulator["shipping_milli"] += row["shipping_milli"]
    if row["discount_ppm"] > 200000:
        accumulator["high_discount_count"] += 1
    if row["profit_milli"] < row["shipping_milli"]:
        accumulator["negative_contribution_count"] += 1


def finalize_metrics(accumulator):
    sales = accumulator["sales_milli"]
    profit = accumulator["profit_milli"]
    shipping = accumulator["shipping_milli"]
    contribution = profit - shipping
    return {
        "row_count": accumulator["row_count"],
        "quantity": accumulator["quantity"],
        "sales_milli": sales,
        "profit_milli": profit,
        "shipping_milli": shipping,
        "contribution_milli": contribution,
        "profit_margin_bp": ratio_bp(profit, sales),
        "contribution_margin_bp": ratio_bp(contribution, sales),
        "high_discount_count": accumulator["high_discount_count"],
        "negative_contribution_count": accumulator[
            "negative_contribution_count"
        ],
    }


def build_aggregates(normalized_rows, effective_rows):
    group_keys = {}
    accumulators = {}
    for name, fields in GROUP_DEFINITIONS:
        keys = set()
        for row in normalized_rows:
            keys.add(tuple(group_value(row, field) for field in fields))
        group_keys[name] = (fields, keys)
        accumulators[name] = {key: new_accumulator() for key in keys}

    for row in effective_rows:
        if not row["active"]:
            continue
        for name, (fields, _keys) in group_keys.items():
            key = tuple(group_value(row, field) for field in fields)
            add_row(accumulators[name][key], row)

    aggregates = {}
    for name, (fields, keys) in group_keys.items():
        items = []
        for key in sorted(keys):
            obj = {field: key[index] for index, field in enumerate(fields)}
            obj.update(finalize_metrics(accumulators[name][key]))
            items.append(obj)
        aggregates[name] = items
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    accumulator = new_accumulator()
    cancelled = 0
    for row in effective_rows:
        if row["active"]:
            add_row(accumulator, row)
        else:
            cancelled += 1

    summary = finalize_metrics(accumulator)
    order_ids = {row["order_id"] for row in normalized_rows}
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - len(order_ids)
    summary["cancelled_row_count"] = cancelled
    summary["event_status_counts"] = dict(
        Counter(item["status"] for item in audit)
    )
    return summary


# ---------------------------------------------------------------------------
# Reconciliation against the two reference summary tables
# ---------------------------------------------------------------------------

SEGMENT_FIELDS = {
    "Order_Count": 0,
    "Total_Sales": 2,
    "Total_Profit": 2,
    "Profit_Margin": 2,
    "Avg_Discount": 4,
}

SHIPPING_FIELDS = {
    "Order_Count": 0,
    "Total_Shipping_Cost": 2,
    "Avg_Shipping_Cost": 2,
    "Avg_Delivery_Days": 2,
}


def segment_actual(rows, field):
    count = len(rows)
    total_sales = sum(row["sales_milli"] for row in rows)
    total_profit = sum(row["profit_milli"] for row in rows)
    if field == "Order_Count":
        return Decimal(count), 0
    if field == "Total_Sales":
        return Decimal(total_sales) / Decimal(1000), 2
    if field == "Total_Profit":
        return Decimal(total_profit) / Decimal(1000), 2
    if field == "Profit_Margin":
        if total_sales == 0:
            return Decimal(0), 2
        return Decimal(total_profit) / Decimal(total_sales) * Decimal(100), 2
    if field == "Avg_Discount":
        if count == 0:
            return Decimal(0), 4
        total_ppm = sum(row["discount_ppm"] for row in rows)
        return (
            Decimal(total_ppm) / Decimal(count) / Decimal(1000000),
            4,
        )
    return None


def shipping_actual(rows, field):
    count = len(rows)
    total_shipping = sum(row["shipping_milli"] for row in rows)
    total_days = sum(row["delivery_days"] for row in rows)
    if field == "Order_Count":
        return Decimal(count), 0
    if field == "Total_Shipping_Cost":
        return Decimal(total_shipping) / Decimal(1000), 2
    if field == "Avg_Shipping_Cost":
        if count == 0:
            return Decimal(0), 2
        return Decimal(total_shipping) / Decimal(count) / Decimal(1000), 2
    if field == "Avg_Delivery_Days":
        if count == 0:
            return Decimal(0), 2
        return Decimal(total_days) / Decimal(count), 2
    return None


def build_reconciliation(input_dir, normalized_rows):
    by_segment = defaultdict(list)
    by_mode = defaultdict(list)
    for row in normalized_rows:
        by_segment[row["segment"]].append(row)
        by_mode[row["ship_mode"]].append(row)

    configs = (
        (
            "segment_performance.csv",
            "Segment",
            SEGMENT_FIELDS,
            segment_actual,
            by_segment,
        ),
        (
            "shipping_by_mode.csv",
            "Ship_Mode",
            SHIPPING_FIELDS,
            shipping_actual,
            by_mode,
        ),
    )

    results = []
    for file_name, group_column, known_fields, calculator, grouped in configs:
        path = os.path.join(input_dir, file_name)
        if not os.path.exists(path):
            continue
        header, records = read_csv_records(path)
        metric_columns = [
            name for name in header if name and name != group_column
        ]
        for record in records:
            group = (record.get(group_column) or "").strip()
            rows = grouped.get(group, [])
            for field in metric_columns:
                if field not in known_fields:
                    continue
                actual_value = calculator(rows, field)
                if actual_value is None:
                    continue
                value, places = actual_value
                actual = format_fixed(value, places)
                reference, _numeric = format_reference(
                    record.get(field), places
                )
                results.append(
                    {
                        "source_file": file_name,
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
# Candidate actions and exact budget optimisation
# ---------------------------------------------------------------------------

def build_candidates(effective_rows, policy):
    shipping_priorities = set(policy.get("shipping_priorities", []))
    shipping_saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    discount_recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_costs = policy.get("shipping_cost_by_market", {})
    discount_costs = policy.get("discount_cost_by_market", {})

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]
        market = row["market"]
        shipping = row["shipping_milli"]
        sales = row["sales_milli"]
        profit = row["profit_milli"]

        if (
            row["ship_mode"] != "Standard Class"
            and row["priority"] in shipping_priorities
        ):
            gain = (shipping * shipping_saving_bp) // 10000
            cost = shipping_costs.get(market)
            if gain > 0 and cost is not None:
                candidates.append(
                    {
                        "action_id": row_key + "|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": int(cost),
                        "gain_milli": int(gain),
                    }
                )

        if (
            row["discount_ppm"] > discount_threshold_ppm
            and profit - shipping < 0
        ):
            gain = (sales * discount_recovery_bp) // 10000
            cost = discount_costs.get(market)
            if gain > 0 and cost is not None:
                candidates.append(
                    {
                        "action_id": row_key + "|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": int(cost),
                        "gain_milli": int(gain),
                    }
                )

    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def optimize_actions(candidates, budget_units):
    """Exact multiple-choice knapsack with the specified lexicographic ties.

    Rows are processed in descending row_key order.  Because every action_id
    starts with its row_key, all candidates of the current row sort before all
    candidates already processed.  Therefore prepending a candidate to the
    selected list preserves the lexicographic ordering of partial solutions,
    and keeping the lexicographically smallest list per (cost, gain) is exact.
    """
    groups = defaultdict(list)
    for candidate in candidates:
        groups[candidate["row_key"]].append(candidate)
    for row_key in groups:
        groups[row_key].sort(key=lambda item: item["action_id"])

    # cost -> (gain, sorted tuple of action_id)
    states = {0: (0, ())}
    for row_key in sorted(groups, reverse=True):
        options = groups[row_key]
        merged = {}

        def put(cost, gain, action_ids):
            current = merged.get(cost)
            if (
                current is None
                or gain > current[0]
                or (gain == current[0] and action_ids < current[1])
            ):
                merged[cost] = (gain, action_ids)

        for cost, (gain, action_ids) in states.items():
            put(cost, gain, action_ids)  # select nothing from this row
            for candidate in options:
                new_cost = cost + candidate["cost_units"]
                if new_cost <= budget_units:
                    put(
                        new_cost,
                        gain + candidate["gain_milli"],
                        (candidate["action_id"],) + action_ids,
                    )

        # A state with higher-or-equal cost and lower-or-equal gain can never
        # become optimal, regardless of the remaining rows.
        pruned = {}
        best_gain = -1
        for cost in sorted(merged):
            gain, action_ids = merged[cost]
            if gain > best_gain:
                pruned[cost] = (gain, action_ids)
                best_gain = gain
        states = pruned

    best_gain = 0
    best_cost = 0
    best_ids = ()
    for cost in sorted(states):
        gain, action_ids = states[cost]
        if gain > best_gain:
            best_gain = gain
            best_cost = cost
            best_ids = action_ids

    return best_gain, best_cost, best_ids


def build_optimization(effective_rows, candidates, policy):
    budget = int(policy.get("budget_units", 0))
    gain, spent, selected_ids = optimize_actions(candidates, budget)
    by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected_actions = [
        by_id[action_id] for action_id in sorted(selected_ids)
    ]
    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"]
        for row in effective_rows
        if row["active"]
    )
    optimization = {
        "budget_units": budget,
        "spent_units": spent,
        "gain_milli": gain,
        "selected_action_ids": sorted(selected_ids),
        "projected_contribution_milli": current_contribution + gain,
    }
    return selected_actions, optimization


# ---------------------------------------------------------------------------
# Input hashes and entry point
# ---------------------------------------------------------------------------

def hash_input_files(input_dir):
    result = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if not os.path.isfile(path) or os.path.islink(path):
            continue
        digest = hashlib.sha256()
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        result[name] = digest.hexdigest()
    return result


def run(input_dir, output_dir):
    products = load_products(os.path.join(input_dir, "product_info.csv"))
    normalized_rows = load_normalized_rows(input_dir, products)

    events = load_events(os.path.join(input_dir, "adjustments.jsonl"))
    event_audit, effective_rows = process_events(normalized_rows, events)

    aggregates = build_aggregates(normalized_rows, effective_rows)
    summary = build_summary(normalized_rows, effective_rows, event_audit)
    reconciliation = build_reconciliation(input_dir, normalized_rows)

    with open(os.path.join(input_dir, "policy.json"), "r", encoding="utf-8") as handle:
        policy = json.load(handle)

    action_candidates = build_candidates(effective_rows, policy)
    selected_actions, optimization = build_optimization(
        effective_rows, action_candidates, policy
    )

    results = {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized_rows,
        "event_audit": event_audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": action_candidates,
        "selected_actions": selected_actions,
        "optimization": optimization,
        "input_sha256": hash_input_files(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    output_path = os.path.join(output_dir, "results.json")
    text = json.dumps(
        results,
        ensure_ascii=False,
        sort_keys=True,
        indent=2,
        allow_nan=False,
    )
    with open(output_path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text + "\n")
    return output_path


def main():
    parser = argparse.ArgumentParser(
        description="Build the 107-V1 order audit and optimisation results."
    )
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args()
    run(args.input, args.output)


if __name__ == "__main__":
    main()
