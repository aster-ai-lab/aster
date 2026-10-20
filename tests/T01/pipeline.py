#!/usr/bin/env python3
"""Global order audit, event processing and budget optimisation (107-v1).

Usage:
    python pipeline.py --input input --output output

Only the Python standard library is used.  All monetary amounts are handled as
integer thousandths ("milli") and discounts as integer millionths ("ppm") so no
binary floating point rounding can leak into the results.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP, localcontext

SCHEMA_VERSION = "107-v1"
ORDER_SUFFIX = "_orders.csv"
CURRENCY_SCALE = 1000          # milli
DISCOUNT_SCALE = 1_000_000     # ppm
BASIS_POINTS = 10000
HIGH_DISCOUNT_PPM = 200000
STANDARD_SHIP_MODE = "Standard Class"
VALID_EVENT_TYPES = ("cancel", "refund", "shipping_credit")


# ---------------------------------------------------------------------------
# numeric helpers
# ---------------------------------------------------------------------------

def parse_scaled(value, scale):
    """Convert a decimal string to an exact integer scaled by ``scale``."""
    text = str(value).strip()
    if text == "":
        return 0
    with localcontext() as ctx:
        ctx.prec = 60
        scaled = Decimal(text) * scale
        return int(scaled.quantize(Decimal(1), rounding=ROUND_HALF_UP))


def parse_milli(value):
    return parse_scaled(value, CURRENCY_SCALE)


def parse_ppm(value):
    return parse_scaled(value, DISCOUNT_SCALE)


def round_half_up_ratio(numerator, denominator, scale):
    """Exact ROUND_HALF_UP of ``numerator / denominator * scale``.

    Ties round away from zero.  Returns ``None`` when the denominator is zero.
    """
    if denominator == 0:
        return None
    num = numerator * scale
    den = denominator
    if den < 0:
        num = -num
        den = -den
    if num >= 0:
        return (2 * num + den) // (2 * den)
    return -((-2 * num + den) // (2 * den))


def ratio_bp(numerator, denominator):
    return round_half_up_ratio(numerator, denominator, BASIS_POINTS)


def format_decimal(value, places):
    """Format an exact Decimal with a fixed number of decimals (half up)."""
    quant = Decimal(1).scaleb(-places)
    with localcontext() as ctx:
        ctx.prec = 60
        rounded = value.quantize(quant, rounding=ROUND_HALF_UP)
    if rounded == 0:
        rounded = abs(rounded)  # avoid "-0.00"
    return f"{rounded:.{places}f}"


# ---------------------------------------------------------------------------
# input loading
# ---------------------------------------------------------------------------

def read_csv_records(path):
    """Return the records of a CSV file, skipping blank physical lines."""
    records = []
    header = None
    with open(path, newline="", encoding="utf-8-sig") as handle:
        for raw in csv.reader(handle):
            if not raw or all((cell or "").strip() == "" for cell in raw):
                continue
            if header is None:
                header = [cell.strip() for cell in raw]
                continue
            records.append(dict(zip(header, raw)))
    return records


def load_product_catalog(input_dir):
    catalog = {}
    for record in read_csv_records(os.path.join(input_dir, "product_info.csv")):
        product_id = record.get("Product ID")
        if product_id in catalog:
            continue
        catalog[product_id] = (
            record.get("Category", ""),
            record.get("Sub-Category", ""),
        )
    return catalog


def list_order_files(input_dir):
    names = []
    for entry in os.scandir(input_dir):
        if entry.is_file() and entry.name.endswith(ORDER_SUFFIX):
            names.append(entry.name)
    return sorted(names)


def load_normalized_rows(input_dir, catalog):
    rows = []
    for source_file in list_order_files(input_dir):
        market = source_file[: -len(ORDER_SUFFIX)]
        records = read_csv_records(os.path.join(input_dir, source_file))
        for index, record in enumerate(records, start=1):
            product_id = record.get("Product ID", "")
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date_text = record.get("Order Date", "").strip()
            ship_date_text = record.get("Ship Date", "").strip()
            order_date = date.fromisoformat(order_date_text)
            ship_date = date.fromisoformat(ship_date_text)
            rows.append({
                "row_key": f"{source_file}:{index:06d}",
                "source_file": source_file,
                "source_row": index,
                "order_id": record.get("Order ID", ""),
                "product_id": product_id,
                "market": market,
                "category": category,
                "sub_category": sub_category,
                "segment": record.get("Segment", ""),
                "ship_mode": record.get("Ship Mode", ""),
                "priority": record.get("Order Priority", ""),
                "order_date": order_date_text,
                "ship_date": ship_date_text,
                "delivery_days": (ship_date - order_date).days,
                "quantity": int(str(record.get("Quantity", "0")).strip() or 0),
                "sales_milli": parse_milli(record.get("Sales", "0")),
                "profit_milli": parse_milli(record.get("Profit", "0")),
                "shipping_milli": parse_milli(record.get("Shipping Cost", "0")),
                "discount_ppm": parse_ppm(record.get("Discount", "0")),
            })
    rows.sort(key=lambda row: row["row_key"])
    return rows


def load_json_file(input_dir, name):
    with open(os.path.join(input_dir, name), encoding="utf-8-sig") as handle:
        return json.load(handle)


def load_events(input_dir):
    """Return non-empty adjustment records as ``(input_line, payload)``."""
    path = os.path.join(input_dir, "adjustments.jsonl")
    records = []
    with open(path, encoding="utf-8-sig") as handle:
        for raw in handle:
            text = raw.strip()
            if not text:
                continue
            records.append((len(records) + 1, json.loads(text)))
    return records


# ---------------------------------------------------------------------------
# section 3: event ledger
# ---------------------------------------------------------------------------

def process_events(rows, events):
    """Apply the de-duplicated, time-ordered event stream.

    Returns ``(audit, effective_rows)``.
    """
    state = {}
    order = []
    for row in rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
            "cancelled": False,
        }
        order.append(row["row_key"])

    first_records = []
    seen_ids = set()
    duplicate_flags = []
    for input_line, payload in events:
        event_id = payload.get("event_id")
        try:
            is_duplicate = event_id in seen_ids
        except TypeError:  # unhashable id can never be a known duplicate key
            is_duplicate = False
        duplicate_flags.append(is_duplicate)
        if not is_duplicate:
            try:
                seen_ids.add(event_id)
            except TypeError:
                pass
            first_records.append(payload)

    def sort_key(payload):
        return (str(payload.get("timestamp", "")), str(payload.get("event_id", "")))

    statuses = {}
    for payload in sorted(first_records, key=sort_key):
        event_id = payload.get("event_id")
        row_key = payload.get("row_key")
        event_type = payload.get("type")
        amount = payload.get("amount_milli")

        if not isinstance(row_key, str) or row_key not in state:
            status = "unknown_row"
        elif event_type not in VALID_EVENT_TYPES:
            status = "unknown_type"
        elif state[row_key]["cancelled"]:
            status = "order_cancelled"
        else:
            row_state = state[row_key]
            is_integer = isinstance(amount, int) and not isinstance(amount, bool)
            if event_type == "cancel":
                valid = is_integer and amount == 0
            elif event_type == "refund":
                valid = is_integer and amount > 0 and amount <= row_state["sales_milli"]
            else:  # shipping_credit
                valid = is_integer and amount > 0 and amount <= row_state["shipping_milli"]
            if not valid:
                status = "invalid_amount"
            else:
                status = "applied"
                if event_type == "cancel":
                    row_state["active"] = False
                    row_state["cancelled"] = True
                    row_state["sales_milli"] = 0
                    row_state["profit_milli"] = 0
                    row_state["shipping_milli"] = 0
                elif event_type == "refund":
                    row_state["sales_milli"] -= amount
                    row_state["profit_milli"] -= amount
                else:
                    row_state["shipping_milli"] -= amount
        try:
            statuses[event_id] = status
        except TypeError:
            statuses[str(event_id)] = status

    audit = []
    for (input_line, payload), is_duplicate in zip(events, duplicate_flags):
        event_id = payload.get("event_id")
        if is_duplicate:
            status = "duplicate_event"
        else:
            try:
                status = statuses[event_id]
            except (KeyError, TypeError):
                status = statuses.get(str(event_id))
        audit.append({
            "input_line": input_line,
            "event_id": event_id,
            "row_key": payload.get("row_key"),
            "status": status,
        })

    effective = []
    for row in rows:
        row_state = state[row["row_key"]]
        clone = dict(row)
        clone["active"] = row_state["active"]
        clone["sales_milli"] = row_state["sales_milli"]
        clone["profit_milli"] = row_state["profit_milli"]
        clone["shipping_milli"] = row_state["shipping_milli"]
        effective.append(clone)
    return audit, effective


# ---------------------------------------------------------------------------
# section 4: aggregates
# ---------------------------------------------------------------------------

def empty_accumulator():
    return {
        "row_count": 0,
        "quantity": 0,
        "sales_milli": 0,
        "profit_milli": 0,
        "shipping_milli": 0,
        "high_discount_count": 0,
        "negative_contribution_count": 0,
    }


def accumulate(accumulator, row):
    accumulator["row_count"] += 1
    accumulator["quantity"] += row["quantity"]
    accumulator["sales_milli"] += row["sales_milli"]
    accumulator["profit_milli"] += row["profit_milli"]
    accumulator["shipping_milli"] += row["shipping_milli"]
    if row["discount_ppm"] > HIGH_DISCOUNT_PPM:
        accumulator["high_discount_count"] += 1
    if row["profit_milli"] < row["shipping_milli"]:
        accumulator["negative_contribution_count"] += 1


def metrics_from_accumulator(accumulator):
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
        "negative_contribution_count": accumulator["negative_contribution_count"],
    }


AGGREGATE_GROUPS = (
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


def build_aggregates(normalized_rows, effective_rows):
    aggregates = {}
    for name, fields in AGGREGATE_GROUPS:
        buckets = {}
        for row in normalized_rows:
            key = group_key(row, fields)
            if key not in buckets:
                buckets[key] = empty_accumulator()
        for row in effective_rows:
            if not row["active"]:
                continue
            accumulate(buckets[group_key(row, fields)], row)
        items = []
        for key in sorted(buckets):
            obj = {}
            for field, value in zip(fields, key):
                obj[field] = value
            obj.update(metrics_from_accumulator(buckets[key]))
            items.append(obj)
        aggregates[name] = items
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    accumulator = empty_accumulator()
    for row in effective_rows:
        if row["active"]:
            accumulate(accumulator, row)
    summary = metrics_from_accumulator(accumulator)
    total_rows = len(normalized_rows)
    unique_orders = len({row["order_id"] for row in normalized_rows})
    status_counts = {}
    for entry in audit:
        status = entry["status"]
        status_counts[status] = status_counts.get(status, 0) + 1
    summary["input_row_count"] = total_rows
    summary["unique_order_id_count"] = unique_orders
    summary["duplicate_order_id_extra_rows"] = total_rows - unique_orders
    summary["cancelled_row_count"] = sum(1 for row in effective_rows if not row["active"])
    summary["event_status_counts"] = {
        key: status_counts[key] for key in sorted(status_counts)
    }
    return summary


# ---------------------------------------------------------------------------
# section 5: reconciliation against the published reference tables
# ---------------------------------------------------------------------------

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


def decimal_ratio(numerator, denominator, multiplier=1):
    if denominator == 0:
        return Decimal(0)
    with localcontext() as ctx:
        ctx.prec = 60
        return Decimal(numerator) * multiplier / Decimal(denominator)


def segment_actual(bucket):
    count = bucket["count"]
    values = {
        "Order_Count": Decimal(count),
        "Total_Sales": Decimal(bucket["sales_milli"]) / CURRENCY_SCALE,
        "Total_Profit": Decimal(bucket["profit_milli"]) / CURRENCY_SCALE,
        "Profit_Margin": (
            decimal_ratio(bucket["profit_milli"], bucket["sales_milli"], 100)
            if bucket["sales_milli"] != 0 else Decimal(0)
        ),
        "Avg_Discount": decimal_ratio(
            bucket["discount_ppm"], count, Decimal(1) / DISCOUNT_SCALE
        ),
    }
    return values


def shipping_actual(bucket):
    count = bucket["count"]
    values = {
        "Order_Count": Decimal(count),
        "Total_Shipping_Cost": Decimal(bucket["shipping_milli"]) / CURRENCY_SCALE,
        "Avg_Shipping_Cost": decimal_ratio(
            bucket["shipping_milli"], count, Decimal(1) / CURRENCY_SCALE
        ),
        "Avg_Delivery_Days": decimal_ratio(bucket["delivery_days"], count),
    }
    return values


def build_reconciliation(input_dir, normalized_rows):
    results = []

    segment_buckets = {}
    for row in normalized_rows:
        bucket = segment_buckets.setdefault(
            row["segment"],
            {"count": 0, "sales_milli": 0, "profit_milli": 0,
             "discount_ppm": 0},
        )
        bucket["count"] += 1
        bucket["sales_milli"] += row["sales_milli"]
        bucket["profit_milli"] += row["profit_milli"]
        bucket["discount_ppm"] += row["discount_ppm"]

    for record in read_csv_records(os.path.join(input_dir, "segment_performance.csv")):
        group = record.get("Segment", "")
        bucket = segment_buckets.get(
            group,
            {"count": 0, "sales_milli": 0, "profit_milli": 0, "discount_ppm": 0},
        )
        actuals = segment_actual(bucket)
        for field, places in SEGMENT_FIELDS:
            actual = format_decimal(actuals[field], places)
            reference = format_decimal(
                Decimal(str(record.get(field, "0")).strip() or "0"), places
            )
            results.append({
                "source_file": "segment_performance.csv",
                "group": group,
                "field": field,
                "actual": actual,
                "reference": reference,
                "match": actual == reference,
            })

    shipping_buckets = {}
    for row in normalized_rows:
        bucket = shipping_buckets.setdefault(
            row["ship_mode"],
            {"count": 0, "shipping_milli": 0, "delivery_days": 0},
        )
        bucket["count"] += 1
        bucket["shipping_milli"] += row["shipping_milli"]
        bucket["delivery_days"] += row["delivery_days"]

    for record in read_csv_records(os.path.join(input_dir, "shipping_by_mode.csv")):
        group = record.get("Ship_Mode", "")
        bucket = shipping_buckets.get(
            group, {"count": 0, "shipping_milli": 0, "delivery_days": 0}
        )
        actuals = shipping_actual(bucket)
        for field, places in SHIPPING_FIELDS:
            actual = format_decimal(actuals[field], places)
            reference = format_decimal(
                Decimal(str(record.get(field, "0")).strip() or "0"), places
            )
            results.append({
                "source_file": "shipping_by_mode.csv",
                "group": group,
                "field": field,
                "actual": actual,
                "reference": reference,
                "match": actual == reference,
            })

    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


# ---------------------------------------------------------------------------
# section 6: candidates and exact budget optimisation
# ---------------------------------------------------------------------------

def policy_int(policy, key, default=0):
    value = policy.get(key, default)
    if isinstance(value, bool):
        return default
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def policy_map(policy, key):
    value = policy.get(key, {})
    return value if isinstance(value, dict) else {}


def build_candidates(effective_rows, policy):
    priorities = set(policy.get("shipping_priorities", []) or [])
    saving_bp = policy_int(policy, "shipping_saving_bp")
    threshold_ppm = policy_int(policy, "discount_threshold_ppm")
    recovery_bp = policy_int(policy, "discount_recovery_bp")
    shipping_costs = policy_map(policy, "shipping_cost_by_market")
    discount_costs = policy_map(policy, "discount_cost_by_market")

    candidates = []
    for row in effective_rows:
        market = row["market"]
        if row["ship_mode"] != STANDARD_SHIP_MODE and row["priority"] in priorities:
            gain = row["shipping_milli"] * saving_bp // BASIS_POINTS
            if gain > 0:
                candidates.append({
                    "action_id": row["row_key"] + "|shipping_switch",
                    "row_key": row["row_key"],
                    "type": "shipping_switch",
                    "cost_units": int(shipping_costs.get(market, 0)),
                    "gain_milli": gain,
                })
        if (row["discount_ppm"] > threshold_ppm
                and row["profit_milli"] - row["shipping_milli"] < 0):
            gain = row["sales_milli"] * recovery_bp // BASIS_POINTS
            if gain > 0:
                candidates.append({
                    "action_id": row["row_key"] + "|discount_review",
                    "row_key": row["row_key"],
                    "type": "discount_review",
                    "cost_units": int(discount_costs.get(market, 0)),
                    "gain_milli": gain,
                })
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def pareto_prune(states):
    """Keep (cost, gain) pairs where gain strictly improves as cost grows."""
    kept = {}
    best = -1
    for cost in sorted(states):
        gain = states[cost]
        if gain > best:
            kept[cost] = gain
            best = gain
    return kept


def optimise(candidates, budget_units):
    """Exact 0/1 knapsack with at most one action per row_key.

    Objectives in priority order: max total gain, min total cost, then the
    lexicographically smallest sorted action_id list.
    """
    budget = max(0, int(budget_units))
    grouped = {}
    for candidate in candidates:
        grouped.setdefault(candidate["row_key"], []).append(candidate)
    row_keys = sorted(grouped)
    options = [sorted(grouped[key], key=lambda item: item["action_id"])
               for key in row_keys]

    # Cost above the sum of per-row maximum costs can never be spent.
    max_total_cost = 0
    for group in options:
        max_total_cost += max(option["cost_units"] for option in group)
    limit = min(budget, max_total_cost)

    # Quick exact case: the budget covers the best option of every row, so every
    # positive-gain row can be taken.
    best_total_cost = 0
    for group in options:
        best = max(group, key=lambda option: (option["gain_milli"], -option["cost_units"]))
        best_total_cost += best["cost_units"]
    if budget >= best_total_cost:
        chosen = []
        for group in options:
            best = max(group, key=lambda option: (option["gain_milli"], -option["cost_units"]))
            chosen.append(best)
        chosen.sort(key=lambda item: item["action_id"])
        return chosen

    # Suffix dynamic programming over exact cost.  Each layer maps cost to the
    # maximum gain reachable with that exact cost from this row onwards.
    count = len(options)
    layers = [None] * (count + 1)
    layers[count] = {0: 0}
    for index in range(count - 1, -1, -1):
        nxt = layers[index + 1]
        current = dict(nxt)
        for option in options[index]:
            cost = option["cost_units"]
            gain = option["gain_milli"]
            if cost < 0 or cost > limit:
                continue
            for suffix_cost, suffix_gain in nxt.items():
                new_cost = suffix_cost + cost
                if new_cost > limit:
                    continue
                new_gain = suffix_gain + gain
                if new_gain > current.get(new_cost, -1):
                    current[new_cost] = new_gain
        layers[index] = pareto_prune(current)

    best_gain = max(layers[0].values()) if layers[0] else 0
    target_cost = min(cost for cost, gain in layers[0].items() if gain == best_gain)

    selected = []
    used_cost = 0
    gained = 0
    for index in range(count):
        group = options[index]
        remaining_gain = best_gain - gained
        chosen = None
        for option in group:
            suffix_cost = target_cost - used_cost - option["cost_units"]
            if suffix_cost < 0:
                continue
            suffix_gain = layers[index + 1].get(suffix_cost)
            if suffix_gain is None:
                continue
            if suffix_gain == remaining_gain - option["gain_milli"]:
                chosen = option
                break
        if chosen is not None:
            selected.append(chosen)
            used_cost += chosen["cost_units"]
            gained += chosen["gain_milli"]

    selected.sort(key=lambda item: item["action_id"])
    return selected


# ---------------------------------------------------------------------------
# orchestration
# ---------------------------------------------------------------------------

def sha256_map(input_dir):
    digests = {}
    for entry in sorted(os.scandir(input_dir), key=lambda item: item.name):
        if not entry.is_file():
            continue
        hasher = hashlib.sha256()
        with open(entry.path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                hasher.update(chunk)
        digests[entry.name] = hasher.hexdigest()
    return digests


def run(input_dir, output_dir):
    catalog = load_product_catalog(input_dir)
    normalized_rows = load_normalized_rows(input_dir, catalog)
    events = load_events(input_dir)
    policy = load_json_file(input_dir, "policy.json")

    audit, effective_rows = process_events(normalized_rows, events)
    aggregates = build_aggregates(normalized_rows, effective_rows)
    summary = build_summary(normalized_rows, effective_rows, audit)
    reconciliation = build_reconciliation(input_dir, normalized_rows)

    candidates = build_candidates(effective_rows, policy)
    budget = policy_int(policy, "budget_units")
    selected = optimise(candidates, budget)

    spent = sum(item["cost_units"] for item in selected)
    gain = sum(item["gain_milli"] for item in selected)
    optimization = {
        "budget_units": budget,
        "spent_units": spent,
        "gain_milli": gain,
        "selected_action_ids": [item["action_id"] for item in selected],
        "projected_contribution_milli": summary["contribution_milli"] + gain,
    }

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
        "optimization": optimization,
        "input_sha256": sha256_map(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    destination = os.path.join(output_dir, "results.json")
    with open(destination, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    return results


def main():
    parser = argparse.ArgumentParser(description="107-v1 order audit pipeline")
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args()
    run(args.input, args.output)


if __name__ == "__main__":
    main()
