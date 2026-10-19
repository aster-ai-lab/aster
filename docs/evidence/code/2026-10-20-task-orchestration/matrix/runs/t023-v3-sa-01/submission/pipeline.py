#!/usr/bin/env python3
"""107-V1 global order audit, event processing and budget optimization.

Standard library only.  Usage:
    python pipeline.py --input input --output output
"""

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, InvalidOperation
from fractions import Fraction

MILLI = 1000
PPM = 1000000
BP = 10000

MARKETS = ("Africa", "Asia_Pacific", "Europe", "LATAM", "USCA")
EVENT_TYPES = ("cancel", "refund", "shipping_credit")


# ---------------------------------------------------------------------------
# numeric helpers (exact integer / Fraction arithmetic, ROUND_HALF_UP)
# ---------------------------------------------------------------------------

def parse_scaled(text, factor):
    """Convert a decimal string to an exact integer scaled by ``factor``."""
    s = text.strip()
    if s == "":
        return 0
    value = Decimal(s) * factor
    if value == value.to_integral_value():
        return int(value)
    # Inputs are documented with at most 3 (money) / 6 (discount) decimals.
    # Fall back to half-up rounding only for out-of-contract extra digits.
    return round_half_up(Fraction(value))


def round_half_up(value):
    """Round a Fraction to the nearest integer, halves away from zero."""
    numerator = value.numerator
    denominator = value.denominator
    sign = -1 if numerator < 0 else 1
    numerator = abs(numerator)
    quotient, remainder = divmod(numerator, denominator)
    if remainder * 2 >= denominator:
        quotient += 1
    return sign * quotient


def format_scaled_int(value, places):
    """Render an integer scaled by 10**places with exactly ``places`` digits."""
    if places == 0:
        return str(value)
    sign = "-" if value < 0 else ""
    digits = str(abs(value)).rjust(places + 1, "0")
    return sign + digits[:-places] + "." + digits[-places:]


def format_fraction(value, places):
    scaled = value * (10 ** places)
    return format_scaled_int(round_half_up(scaled), places)


def ratio_bp(numerator, denominator):
    """ROUND_HALF_UP(numerator / denominator * 10000); None when den == 0."""
    if denominator == 0:
        return None
    return round_half_up(Fraction(numerator, denominator) * BP)


# ---------------------------------------------------------------------------
# CSV helpers
# ---------------------------------------------------------------------------

def read_csv_records(path):
    """Return (header, data records).  Blank physical rows are skipped."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        header = None
        records = []
        for row in reader:
            if not row or all(cell.strip() == "" for cell in row):
                continue
            if header is None:
                header = [cell.strip() for cell in row]
                continue
            records.append(row)
    return header, records


def cell_value(record, index, name):
    position = index.get(name)
    if position is None or position >= len(record):
        return ""
    return record[position].strip()


def load_product_catalog(path):
    header, records = read_csv_records(path)
    index = {name: i for i, name in enumerate(header)}
    catalog = {}
    for record in records:
        product_id = cell_value(record, index, "Product ID")
        catalog[product_id] = (
            cell_value(record, index, "Category"),
            cell_value(record, index, "Sub-Category"),
        )
    return catalog


def market_from_filename(filename):
    base = os.path.basename(filename)
    suffix = "_orders.csv"
    if base.endswith(suffix):
        return base[: -len(suffix)]
    return os.path.splitext(base)[0]


def load_market_rows(path, market, catalog):
    header, records = read_csv_records(path)
    index = {name: i for i, name in enumerate(header)}
    rows = []
    for sequence, record in enumerate(records, 1):
        def cell(name):
            return cell_value(record, index, name)

        product_id = cell("Product ID")
        category, sub_category = catalog.get(product_id, ("", ""))
        order_date = cell("Order Date")
        ship_date = cell("Ship Date")
        delivery_days = (
            date.fromisoformat(ship_date) - date.fromisoformat(order_date)
        ).days
        rows.append({
            "row_key": "%s:%06d" % (os.path.basename(path), sequence),
            "source_file": os.path.basename(path),
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
            "quantity": int(Decimal(cell("Quantity") or "0")),
            "sales_milli": parse_scaled(cell("Sales"), MILLI),
            "profit_milli": parse_scaled(cell("Profit"), MILLI),
            "shipping_milli": parse_scaled(cell("Shipping Cost"), MILLI),
            "discount_ppm": parse_scaled(cell("Discount"), PPM),
        })
    return rows


# ---------------------------------------------------------------------------
# metric helpers
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


def accumulate(acc, row):
    acc["row_count"] += 1
    acc["quantity"] += row["quantity"]
    acc["sales_milli"] += row["sales_milli"]
    acc["profit_milli"] += row["profit_milli"]
    acc["shipping_milli"] += row["shipping_milli"]
    if row["discount_ppm"] > 200000:
        acc["high_discount_count"] += 1
    if row["profit_milli"] < row["shipping_milli"]:
        acc["negative_contribution_count"] += 1


def metrics_from_accumulator(acc):
    sales = acc["sales_milli"]
    profit = acc["profit_milli"]
    shipping = acc["shipping_milli"]
    contribution = profit - shipping
    return {
        "row_count": acc["row_count"],
        "quantity": acc["quantity"],
        "sales_milli": sales,
        "profit_milli": profit,
        "shipping_milli": shipping,
        "contribution_milli": contribution,
        "profit_margin_bp": ratio_bp(profit, sales),
        "contribution_margin_bp": ratio_bp(contribution, sales),
        "high_discount_count": acc["high_discount_count"],
        "negative_contribution_count": acc["negative_contribution_count"],
    }


def month_of(order_date):
    return order_date[:7]


# ---------------------------------------------------------------------------
# events
# ---------------------------------------------------------------------------

def load_adjustments(path):
    """Return list of (input_line, payload, duplicate_flag) in file order."""
    entries = []
    with open(path, "r", encoding="utf-8-sig") as handle:
        for line in handle:
            if line.strip() == "":
                continue
            entries.append(json.loads(line))
    seen = set()
    result = []
    for input_line, payload in enumerate(entries, 1):
        event_id = payload.get("event_id")
        duplicate = event_id in seen
        if not duplicate:
            seen.add(event_id)
        result.append((input_line, payload, duplicate))
    return result


def is_int_amount(value):
    return type(value) is int


def process_events(entries, normalized_rows):
    state = {}
    for row in normalized_rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    status_by_line = {}
    kept = [
        (input_line, payload)
        for input_line, payload, duplicate in entries
        if not duplicate
    ]
    kept.sort(key=lambda item: (item[1].get("timestamp", ""), item[1].get("event_id", "")))

    for input_line, payload in kept:
        row_key = payload.get("row_key")
        event_type = payload.get("type")
        amount = payload.get("amount_milli")
        current = state.get(row_key)
        if current is None:
            status = "unknown_row"
        elif event_type not in EVENT_TYPES:
            status = "unknown_type"
        elif not current["active"]:
            status = "order_cancelled"
        elif event_type == "cancel":
            if is_int_amount(amount) and amount == 0:
                current["active"] = False
                current["sales_milli"] = 0
                current["profit_milli"] = 0
                current["shipping_milli"] = 0
                status = "applied"
            else:
                status = "invalid_amount"
        elif event_type == "refund":
            if (
                is_int_amount(amount)
                and amount > 0
                and amount <= current["sales_milli"]
            ):
                current["sales_milli"] -= amount
                current["profit_milli"] -= amount
                status = "applied"
            else:
                status = "invalid_amount"
        else:  # shipping_credit
            if (
                is_int_amount(amount)
                and amount > 0
                and amount <= current["shipping_milli"]
            ):
                current["shipping_milli"] -= amount
                status = "applied"
            else:
                status = "invalid_amount"
        status_by_line[input_line] = status

    audit = []
    for input_line, payload, duplicate in entries:
        audit.append({
            "input_line": input_line,
            "event_id": payload.get("event_id"),
            "row_key": payload.get("row_key"),
            "status": "duplicate_event" if duplicate else status_by_line[input_line],
        })
    return state, audit


# ---------------------------------------------------------------------------
# reconciliation
# ---------------------------------------------------------------------------

def format_reference(text, places):
    s = (text or "").strip()
    try:
        value = Fraction(Decimal(s))
    except (InvalidOperation, ValueError):
        return s
    return format_scaled_int(round_half_up(value * (10 ** places)), places)


def reconcile_segment(path, rows):
    header, records = read_csv_records(path)
    index = {name: i for i, name in enumerate(header)}
    fields = {
        "Order_Count": 0,
        "Total_Sales": 2,
        "Total_Profit": 2,
        "Profit_Margin": 2,
        "Avg_Discount": 4,
    }

    references = {}
    for record in records:
        group = cell_value(record, index, "Segment")
        references[group] = {
            name: cell_value(record, index, name) for name in fields
        }

    # Recompute from pre-event detail, keyed by segment.
    actual = {}
    for row in rows:
        segment = row["segment"]
        if segment not in actual:
            actual[segment] = {"count": 0, "sales": 0, "profit": 0, "discount": 0}
        acc = actual[segment]
        acc["count"] += 1
        acc["sales"] += row["sales_milli"]
        acc["profit"] += row["profit_milli"]
        acc["discount"] += row["discount_ppm"]

    result = []
    for group, reference_values in references.items():
        acc = actual.get(group, {"count": 0, "sales": 0, "profit": 0, "discount": 0})
        count = acc["count"]
        values = {
            "Order_Count": format_fraction(Fraction(acc["count"]), 0),
            "Total_Sales": format_fraction(Fraction(acc["sales"], MILLI), 2),
            "Total_Profit": format_fraction(Fraction(acc["profit"], MILLI), 2),
            "Profit_Margin": format_fraction(
                Fraction(acc["profit"], acc["sales"]) * 100
                if acc["sales"] != 0 else Fraction(0),
                2,
            ),
            "Avg_Discount": format_fraction(
                Fraction(acc["discount"], count * PPM) if count else Fraction(0),
                4,
            ),
        }
        for name, places in fields.items():
            reference = format_reference(reference_values.get(name, ""), places)
            result.append({
                "source_file": "segment_performance.csv",
                "group": group,
                "field": name,
                "actual": values[name],
                "reference": reference,
                "match": values[name] == reference,
            })
    return result


def reconcile_shipping(path, rows):
    header, records = read_csv_records(path)
    index = {name: i for i, name in enumerate(header)}
    fields = {
        "Order_Count": 0,
        "Total_Shipping_Cost": 2,
        "Avg_Shipping_Cost": 2,
        "Avg_Delivery_Days": 2,
    }

    references = {}
    for record in records:
        group = cell_value(record, index, "Ship_Mode")
        references[group] = {
            name: cell_value(record, index, name) for name in fields
        }

    actual = {}
    for row in rows:
        ship_mode = row["ship_mode"]
        if ship_mode not in actual:
            actual[ship_mode] = {"count": 0, "shipping": 0, "days": 0}
        acc = actual[ship_mode]
        acc["count"] += 1
        acc["shipping"] += row["shipping_milli"]
        acc["days"] += row["delivery_days"]

    result = []
    for group, reference_values in references.items():
        acc = actual.get(group, {"count": 0, "shipping": 0, "days": 0})
        count = acc["count"]
        values = {
            "Order_Count": format_fraction(Fraction(acc["count"]), 0),
            "Total_Shipping_Cost": format_fraction(
                Fraction(acc["shipping"], MILLI), 2
            ),
            "Avg_Shipping_Cost": format_fraction(
                Fraction(acc["shipping"], count * MILLI) if count else Fraction(0), 2
            ),
            "Avg_Delivery_Days": format_fraction(
                Fraction(acc["days"], count) if count else Fraction(0), 2
            ),
        }
        for name, places in fields.items():
            reference = format_reference(reference_values.get(name, ""), places)
            result.append({
                "source_file": "shipping_by_mode.csv",
                "group": group,
                "field": name,
                "actual": values[name],
                "reference": reference,
                "match": values[name] == reference,
            })
    return result


# ---------------------------------------------------------------------------
# optimization
# ---------------------------------------------------------------------------

def build_candidates(effective_rows, policy):
    priorities = set(policy.get("shipping_priorities", []))
    saving_bp = policy.get("shipping_saving_bp", 0)
    threshold_ppm = policy.get("discount_threshold_ppm", 0)
    recovery_bp = policy.get("discount_recovery_bp", 0)
    shipping_costs = policy.get("shipping_cost_by_market", {})
    discount_costs = policy.get("discount_cost_by_market", {})

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]
        market = row["market"]
        shipping = row["shipping_milli"]
        sales = row["sales_milli"]
        contribution = row["profit_milli"] - shipping

        if row["ship_mode"] != "Standard Class" and row["priority"] in priorities:
            gain = (shipping * saving_bp) // BP
            if gain > 0:
                candidates.append({
                    "action_id": row_key + "|shipping_switch",
                    "row_key": row_key,
                    "type": "shipping_switch",
                    "cost_units": shipping_costs[market],
                    "gain_milli": gain,
                })

        if row["discount_ppm"] > threshold_ppm and contribution < 0:
            gain = (sales * recovery_bp) // BP
            if gain > 0:
                candidates.append({
                    "action_id": row_key + "|discount_review",
                    "row_key": row_key,
                    "type": "discount_review",
                    "cost_units": discount_costs[market],
                    "gain_milli": gain,
                })

    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def prune_states(states):
    """Keep the best state per cost, then drop dominated (cost, gain) pairs."""
    best = {}
    for cost, (gain, ids) in states.items():
        current = best.get(cost)
        if (
            current is None
            or gain > current[0]
            or (gain == current[0] and ids < current[1])
        ):
            best[cost] = (gain, ids)
    result = {}
    best_gain = None
    for cost in sorted(best):
        gain, ids = best[cost]
        if best_gain is None or gain > best_gain:
            result[cost] = (gain, ids)
            best_gain = gain
    return result


def optimize(candidates, budget):
    by_row = {}
    for candidate in candidates:
        by_row.setdefault(candidate["row_key"], []).append(candidate)

    upper_bound = 0
    for row_key in by_row:
        upper_bound += max(c["cost_units"] for c in by_row[row_key])
    cap = min(budget, upper_bound)

    states = {0: (0, ())}
    for row_key in sorted(by_row):
        options = sorted(by_row[row_key], key=lambda c: c["action_id"])
        new_states = dict(states)
        for option in options:
            cost = option["cost_units"]
            gain = option["gain_milli"]
            action_id = option["action_id"]
            for old_cost, (old_gain, old_ids) in states.items():
                new_cost = old_cost + cost
                if new_cost > cap:
                    continue
                candidate_state = (old_gain + gain, old_ids + (action_id,))
                current = new_states.get(new_cost)
                if (
                    current is None
                    or candidate_state[0] > current[0]
                    or (
                        candidate_state[0] == current[0]
                        and candidate_state[1] < current[1]
                    )
                ):
                    new_states[new_cost] = candidate_state
        states = prune_states(new_states)

    best_gain = max(state[0] for state in states.values())
    choices = [
        (cost, gain, ids)
        for cost, (gain, ids) in states.items()
        if gain == best_gain
    ]
    spent, gain, ids = min(choices, key=lambda item: (item[0], item[2]))
    return spent, gain, list(ids)


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def sha256_of_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_results(input_dir):
    order_files = sorted(
        name
        for name in os.listdir(input_dir)
        if name.endswith("_orders.csv")
        and os.path.isfile(os.path.join(input_dir, name))
    )

    catalog = load_product_catalog(os.path.join(input_dir, "product_info.csv"))

    normalized_rows = []
    for filename in order_files:
        market = market_from_filename(filename)
        normalized_rows.extend(
            load_market_rows(os.path.join(input_dir, filename), market, catalog)
        )
    normalized_rows.sort(key=lambda row: row["row_key"])

    entries = load_adjustments(os.path.join(input_dir, "adjustments.jsonl"))
    state, event_audit = process_events(entries, normalized_rows)

    effective_rows = []
    for row in normalized_rows:
        current = state[row["row_key"]]
        effective = dict(row)
        effective["active"] = current["active"]
        effective["sales_milli"] = current["sales_milli"]
        effective["profit_milli"] = current["profit_milli"]
        effective["shipping_milli"] = current["shipping_milli"]
        effective_rows.append(effective)

    # ---- aggregates -----------------------------------------------------
    group_definitions = (
        ("market", ("market",), lambda row: (row["market"],)),
        (
            "market_category",
            ("market", "category"),
            lambda row: (row["market"], row["category"]),
        ),
        ("segment", ("segment",), lambda row: (row["segment"],)),
        ("ship_mode", ("ship_mode",), lambda row: (row["ship_mode"],)),
        (
            "market_month",
            ("market", "month"),
            lambda row: (row["market"], month_of(row["order_date"])),
        ),
    )

    aggregates = {}
    for list_name, field_names, key_function in group_definitions:
        keys = set()
        for row in normalized_rows:
            keys.add(key_function(row))
        accumulators = {key: empty_accumulator() for key in keys}
        for row in effective_rows:
            if not row["active"]:
                continue
            accumulate(accumulators[key_function(row)], row)
        items = []
        for key in sorted(keys):
            item = {name: value for name, value in zip(field_names, key)}
            item.update(metrics_from_accumulator(accumulators[key]))
            items.append(item)
        aggregates[list_name] = items

    # ---- summary --------------------------------------------------------
    summary_acc = empty_accumulator()
    for row in effective_rows:
        if row["active"]:
            accumulate(summary_acc, row)
    summary = metrics_from_accumulator(summary_acc)
    order_ids = {row["order_id"] for row in normalized_rows}
    status_counts = {}
    for entry in event_audit:
        status_counts[entry["status"]] = status_counts.get(entry["status"], 0) + 1
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - len(order_ids)
    summary["cancelled_row_count"] = sum(
        1 for row in effective_rows if not row["active"]
    )
    summary["event_status_counts"] = {
        status: status_counts[status] for status in sorted(status_counts)
    }

    # ---- reconciliation -------------------------------------------------
    reconciliation = []
    reconciliation.extend(
        reconcile_segment(
            os.path.join(input_dir, "segment_performance.csv"), normalized_rows
        )
    )
    reconciliation.extend(
        reconcile_shipping(
            os.path.join(input_dir, "shipping_by_mode.csv"), normalized_rows
        )
    )
    reconciliation.sort(
        key=lambda item: (item["source_file"], item["group"], item["field"])
    )

    # ---- optimization ---------------------------------------------------
    with open(os.path.join(input_dir, "policy.json"), "r", encoding="utf-8-sig") as handle:
        policy = json.load(handle)
    budget = policy.get("budget_units", 0)
    candidates = build_candidates(effective_rows, policy)
    spent, gain, selected_ids = optimize(candidates, budget)
    candidate_by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected_actions = [candidate_by_id[action_id] for action_id in selected_ids]
    total_contribution = sum(
        row["profit_milli"] - row["shipping_milli"]
        for row in effective_rows
        if row["active"]
    )
    optimization = {
        "budget_units": budget,
        "spent_units": spent,
        "gain_milli": gain,
        "selected_action_ids": selected_ids,
        "projected_contribution_milli": total_contribution + gain,
    }

    # ---- input hashes ---------------------------------------------------
    input_sha256 = {}
    for name in sorted(os.listdir(input_dir)):
        path = os.path.join(input_dir, name)
        if os.path.isfile(path):
            input_sha256[name] = sha256_of_file(path)

    return {
        "schema_version": "107-v1",
        "normalized_rows": normalized_rows,
        "event_audit": event_audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected_actions,
        "optimization": optimization,
        "input_sha256": input_sha256,
    }


def main():
    parser = argparse.ArgumentParser(description="107-V1 order audit pipeline")
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args()

    results = build_results(args.input)
    os.makedirs(args.output, exist_ok=True)
    output_path = os.path.join(args.output, "results.json")
    with open(output_path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


if __name__ == "__main__":
    main()
