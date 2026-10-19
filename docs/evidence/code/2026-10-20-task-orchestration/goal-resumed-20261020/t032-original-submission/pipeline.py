#!/usr/bin/env python3
"""Global order audit, after-sales event ledger and budget optimisation (schema 107-v1).

Standard library only.  Usage:

    python pipeline.py --input input --output output
"""

import argparse
import csv
import hashlib
import json
import os
from datetime import date
from decimal import Decimal, ROUND_HALF_UP
from fractions import Fraction

SCHEMA_VERSION = "107-v1"

MILLI_SCALE = Decimal(1000)
PPM_SCALE = Decimal(1000000)

EVENT_TYPES = ("cancel", "refund", "shipping_credit")


# --------------------------------------------------------------------------- #
# exact numeric helpers
# --------------------------------------------------------------------------- #
def _to_decimal(raw):
    text = "" if raw is None else str(raw).strip()
    if not text:
        return Decimal(0)
    return Decimal(text)


def to_milli(raw):
    """Decimal currency text -> integer thousandths, exact for <=3 decimals."""
    return int((_to_decimal(raw) * MILLI_SCALE).to_integral_value(rounding=ROUND_HALF_UP))


def to_ppm(raw):
    """Decimal discount text -> integer parts per million, exact for <=6 decimals."""
    return int((_to_decimal(raw) * PPM_SCALE).to_integral_value(rounding=ROUND_HALF_UP))


def to_int(raw):
    text = "" if raw is None else str(raw).strip()
    if not text:
        return 0
    return int(Decimal(text))


def round_half_up(value, decimals):
    """Round an exact Fraction half away from zero, returning an integer of 10**-decimals units."""
    scaled = Fraction(value) * (10 ** decimals)
    num, den = scaled.numerator, scaled.denominator
    if num >= 0:
        quot, rem = divmod(num, den)
        if 2 * rem >= den:
            quot += 1
        return quot
    quot, rem = divmod(-num, den)
    if 2 * rem >= den:
        quot += 1
    return -quot


def format_fixed(value, decimals):
    """Format an exact value as a fixed-decimal string using ROUND_HALF_UP."""
    units = round_half_up(Fraction(value), decimals)
    sign = "-" if units < 0 else ""
    digits = str(abs(units))
    if decimals == 0:
        return sign + digits
    digits = digits.zfill(decimals + 1)
    return sign + digits[:-decimals] + "." + digits[-decimals:]


def format_decimal_text(raw, decimals):
    """Reference table cell -> fixed-decimal string (same scale as the computed value)."""
    text = "" if raw is None else str(raw).strip()
    if not text:
        return ""
    try:
        value = Decimal(text)
    except Exception:
        return text
    return format_fixed(Fraction(value), decimals)


def ratio_bp(numerator, denominator):
    if denominator == 0:
        return None
    return round_half_up(Fraction(numerator, denominator) * 10000, 0)


# --------------------------------------------------------------------------- #
# input readers
# --------------------------------------------------------------------------- #
def read_csv_table(path):
    """Return (header, records).  Blank rows are skipped; the BOM is tolerated."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        raw_rows = list(csv.reader(handle))
    if not raw_rows:
        return [], []
    header = [cell.strip() for cell in raw_rows[0]]
    records = []
    for raw in raw_rows[1:]:
        if not raw or all(not str(cell).strip() for cell in raw):
            continue
        record = {}
        for idx, name in enumerate(header):
            if not name:
                continue
            record[name] = str(raw[idx]).strip() if idx < len(raw) else ""
        records.append(record)
    return header, records


def load_products(input_dir):
    path = os.path.join(input_dir, "product_info.csv")
    products = {}
    if not os.path.isfile(path):
        return products
    _, records = read_csv_table(path)
    for record in records:
        product_id = record.get("Product ID", "")
        if product_id and product_id not in products:
            products[product_id] = (
                record.get("Category", ""),
                record.get("Sub-Category", ""),
            )
    return products


def parse_iso_date(text):
    try:
        return date(int(text[0:4]), int(text[5:7]), int(text[8:10]))
    except (TypeError, ValueError, IndexError):
        return None


def load_normalized(input_dir):
    products = load_products(input_dir)
    rows = []
    names = sorted(
        name
        for name in os.listdir(input_dir)
        if name.endswith("_orders.csv") and os.path.isfile(os.path.join(input_dir, name))
    )
    for name in names:
        market = name[: -len("_orders.csv")]
        _, records = read_csv_table(os.path.join(input_dir, name))
        for seq, record in enumerate(records, start=1):
            product_id = record.get("Product ID", "")
            category, sub_category = products.get(product_id, ("", ""))
            order_date = record.get("Order Date", "")
            ship_date = record.get("Ship Date", "")
            start = parse_iso_date(order_date)
            end = parse_iso_date(ship_date)
            delivery_days = (end - start).days if start and end else 0
            rows.append(
                {
                    "row_key": "%s:%06d" % (name, seq),
                    "source_file": name,
                    "source_row": seq,
                    "order_id": record.get("Order ID", ""),
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": record.get("Segment", ""),
                    "ship_mode": record.get("Ship Mode", ""),
                    "priority": record.get("Order Priority", ""),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": delivery_days,
                    "quantity": to_int(record.get("Quantity", "")),
                    "sales_milli": to_milli(record.get("Sales", "")),
                    "profit_milli": to_milli(record.get("Profit", "")),
                    "shipping_milli": to_milli(record.get("Shipping Cost", "")),
                    "discount_ppm": to_ppm(record.get("Discount", "")),
                }
            )
    rows.sort(key=lambda row: row["row_key"])
    return rows


def load_policy(input_dir):
    path = os.path.join(input_dir, "policy.json")
    with open(path, "r", encoding="utf-8-sig") as handle:
        return json.load(handle)


def load_event_records(input_dir):
    path = os.path.join(input_dir, "adjustments.jsonl")
    records = []
    if not os.path.isfile(path):
        return records
    with open(path, "r", encoding="utf-8-sig") as handle:
        text = handle.read()
    for line in text.splitlines():
        if not line.strip():
            continue
        records.append(json.loads(line))
    return records


# --------------------------------------------------------------------------- #
# event ledger
# --------------------------------------------------------------------------- #
def is_int_amount(value):
    return isinstance(value, int) and not isinstance(value, bool)


def apply_events(normalized_rows, event_records):
    """Dedupe, replay in (timestamp, event_id) order and build audit + effective rows."""
    states = {}
    for row in normalized_rows:
        states[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    unique = {}
    duplicate_line = {}
    for line_no, record in enumerate(event_records, start=1):
        event_id = record.get("event_id")
        if event_id in unique:
            duplicate_line[line_no] = True
        else:
            unique[event_id] = record

    executed_status = {}
    ordered = sorted(
        unique.items(),
        key=lambda item: (str(item[1].get("timestamp", "")), str(item[0])),
    )
    for event_id, record in ordered:
        row_key = record.get("row_key")
        event_type = record.get("type")
        amount = record.get("amount_milli")
        state = states.get(row_key) if isinstance(row_key, str) else None
        status = None
        if state is None:
            status = "unknown_row"
        elif event_type not in EVENT_TYPES:
            status = "unknown_type"
        elif not state["active"]:
            status = "order_cancelled"
        elif not is_int_amount(amount):
            status = "invalid_amount"
        elif event_type == "cancel":
            if amount != 0:
                status = "invalid_amount"
            else:
                state["active"] = False
                state["sales_milli"] = 0
                state["profit_milli"] = 0
                state["shipping_milli"] = 0
                status = "applied"
        elif event_type == "refund":
            if amount <= 0 or amount > state["sales_milli"]:
                status = "invalid_amount"
            else:
                state["sales_milli"] -= amount
                state["profit_milli"] -= amount
                status = "applied"
        else:  # shipping_credit
            if amount <= 0 or amount > state["shipping_milli"]:
                status = "invalid_amount"
            else:
                state["shipping_milli"] -= amount
                status = "applied"
        executed_status[event_id] = status

    audit = []
    for line_no, record in enumerate(event_records, start=1):
        event_id = record.get("event_id")
        if line_no in duplicate_line:
            status = "duplicate_event"
        else:
            status = executed_status.get(event_id)
        audit.append(
            {
                "input_line": line_no,
                "event_id": event_id,
                "row_key": record.get("row_key"),
                "status": status,
            }
        )

    effective = []
    for row in normalized_rows:
        state = states[row["row_key"]]
        copy = dict(row)
        copy["sales_milli"] = state["sales_milli"]
        copy["profit_milli"] = state["profit_milli"]
        copy["shipping_milli"] = state["shipping_milli"]
        copy["active"] = state["active"]
        effective.append(copy)
    return audit, effective


# --------------------------------------------------------------------------- #
# metrics
# --------------------------------------------------------------------------- #
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


GROUP_DEFINITIONS = (
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
        lambda row: (row["market"], row["order_date"][:7]),
    ),
)


def build_aggregates(normalized_rows, effective_rows):
    aggregate_names = tuple(name for name, _, _ in GROUP_DEFINITIONS)
    groups = {name: {} for name in aggregate_names}

    for row in normalized_rows:
        for name, fields, key_fn in GROUP_DEFINITIONS:
            key = key_fn(row)
            groups[name].setdefault(key, [])

    active_by_key = {name: {} for name in aggregate_names}
    for row in effective_rows:
        if not row["active"]:
            continue
        for name, fields, key_fn in GROUP_DEFINITIONS:
            key = key_fn(row)
            groups[name].setdefault(key, [])
            active_by_key[name].setdefault(key, []).append(row)

    aggregates = {}
    for name, fields, _ in GROUP_DEFINITIONS:
        entries = []
        for key in sorted(groups[name], key=lambda item: tuple(str(part) for part in item)):
            entry = {}
            for field, value in zip(fields, key):
                entry[field] = value
            entry.update(compute_metrics(active_by_key[name].get(key, [])))
            entries.append(entry)
        aggregates[name] = entries
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    active_rows = [row for row in effective_rows if row["active"]]
    summary = compute_metrics(active_rows)
    order_ids = {row["order_id"] for row in normalized_rows}
    status_counts = {}
    for entry in audit:
        status_counts[entry["status"]] = status_counts.get(entry["status"], 0) + 1
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - len(order_ids)
    summary["cancelled_row_count"] = len(normalized_rows) - len(active_rows)
    summary["event_status_counts"] = status_counts
    return summary


# --------------------------------------------------------------------------- #
# reconciliation of the two reference summary tables
# --------------------------------------------------------------------------- #
def _group_facts(rows, key_field):
    facts = {}
    for row in rows:
        key = row[key_field]
        entry = facts.setdefault(
            key,
            {
                "count": 0,
                "sales": 0,
                "profit": 0,
                "shipping": 0,
                "discount_ppm": 0,
                "delivery_days": 0,
            },
        )
        entry["count"] += 1
        entry["sales"] += row["sales_milli"]
        entry["profit"] += row["profit_milli"]
        entry["shipping"] += row["shipping_milli"]
        entry["discount_ppm"] += row["discount_ppm"]
        entry["delivery_days"] += row["delivery_days"]
    return facts


def _segment_values(facts):
    values = {}
    for group, fact in facts.items():
        count = fact["count"]
        values[group] = {
            "Order_Count": (count, 0),
            "Total_Sales": (Fraction(fact["sales"], 1000), 2),
            "Total_Profit": (Fraction(fact["profit"], 1000), 2),
            "Profit_Margin": (
                Fraction(fact["profit"], fact["sales"]) * 100 if fact["sales"] else Fraction(0),
                2,
            ),
            "Avg_Discount": (
                Fraction(fact["discount_ppm"], 1000000 * count) if count else Fraction(0),
                4,
            ),
        }
    return values


def _shipping_values(facts):
    values = {}
    for group, fact in facts.items():
        count = fact["count"]
        values[group] = {
            "Order_Count": (count, 0),
            "Total_Shipping_Cost": (Fraction(fact["shipping"], 1000), 2),
            "Avg_Shipping_Cost": (
                Fraction(fact["shipping"], 1000 * count) if count else Fraction(0),
                2,
            ),
            "Avg_Delivery_Days": (
                Fraction(fact["delivery_days"], count) if count else Fraction(0),
                2,
            ),
        }
    return values


RECONCILIATION_SPECS = (
    ("segment_performance.csv", "Segment", "segment", _segment_values),
    ("shipping_by_mode.csv", "Ship_Mode", "ship_mode", _shipping_values),
)


def build_reconciliation(input_dir, normalized_rows):
    results = []
    for source_file, key_field, row_field, value_builder in RECONCILIATION_SPECS:
        path = os.path.join(input_dir, source_file)
        if not os.path.isfile(path):
            continue
        facts = _group_facts(normalized_rows, row_field)
        values = value_builder(facts)
        empty = {
            name: (Fraction(0), decimals) for name, decimals in _reference_fields(source_file)
        }
        _, records = read_csv_table(path)
        for record in records:
            group = record.get(key_field, "")
            computed = values.get(group, empty)
            for field in sorted(computed):
                exact, decimals = computed[field]
                actual = format_fixed(exact, decimals)
                reference = format_decimal_text(record.get(field, ""), decimals)
                results.append(
                    {
                        "source_file": source_file,
                        "group": group,
                        "field": field,
                        "actual": actual,
                        "reference": reference,
                        "match": actual == reference,
                    }
                )
    results.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return results


def _reference_fields(source_file):
    if source_file == "segment_performance.csv":
        return (
            ("Order_Count", 0),
            ("Total_Sales", 2),
            ("Total_Profit", 2),
            ("Profit_Margin", 2),
            ("Avg_Discount", 4),
        )
    return (
        ("Order_Count", 0),
        ("Total_Shipping_Cost", 2),
        ("Avg_Shipping_Cost", 2),
        ("Avg_Delivery_Days", 2),
    )


# --------------------------------------------------------------------------- #
# candidate actions and exact optimisation
# --------------------------------------------------------------------------- #
def build_candidates(effective_rows, policy):
    shipping_priorities = set(policy.get("shipping_priorities", []))
    shipping_saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    discount_recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_cost_by_market = policy.get("shipping_cost_by_market", {})
    discount_cost_by_market = policy.get("discount_cost_by_market", {})

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]
        market = row["market"]
        shipping = row["shipping_milli"]
        if row["ship_mode"] != "Standard Class" and row["priority"] in shipping_priorities:
            gain = shipping * shipping_saving_bp // 10000
            if gain > 0 and market in shipping_cost_by_market:
                candidates.append(
                    {
                        "action_id": row_key + "|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": int(shipping_cost_by_market[market]),
                        "gain_milli": gain,
                    }
                )
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold_ppm and contribution < 0:
            gain = row["sales_milli"] * discount_recovery_bp // 10000
            if gain > 0 and market in discount_cost_by_market:
                candidates.append(
                    {
                        "action_id": row_key + "|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": int(discount_cost_by_market[market]),
                        "gain_milli": gain,
                    }
                )
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def _insert_sorted(items, value):
    position = 0
    while position < len(items) and items[position] < value:
        position += 1
    return items[:position] + (value,) + items[position:]


def select_actions(candidates, budget):
    """Exact multiple-choice knapsack with the three-level tie-break from the spec."""
    by_row = {}
    for candidate in candidates:
        by_row.setdefault(candidate["row_key"], []).append(candidate)

    # cost -> (total_gain, sorted tuple of chosen action_ids)
    states = {0: (0, ())}
    for row_key in sorted(by_row):
        options = by_row[row_key]
        merged = dict(states)
        for cost in sorted(states):
            gain, chosen = states[cost]
            for candidate in options:
                new_cost = cost + candidate["cost_units"]
                if new_cost > budget:
                    continue
                new_gain = gain + candidate["gain_milli"]
                new_chosen = _insert_sorted(chosen, candidate["action_id"])
                current = merged.get(new_cost)
                if (
                    current is None
                    or new_gain > current[0]
                    or (new_gain == current[0] and new_chosen < current[1])
                ):
                    merged[new_cost] = (new_gain, new_chosen)
        pruned = {}
        best_gain = None
        for cost in sorted(merged):
            gain, chosen = merged[cost]
            if best_gain is None or gain > best_gain:
                pruned[cost] = (gain, chosen)
                best_gain = gain
        states = pruned

    best_cost = 0
    best_gain = 0
    best_chosen = ()
    for cost in sorted(states):
        gain, chosen = states[cost]
        if gain > best_gain or (gain == best_gain and chosen < best_chosen):
            best_gain, best_chosen, best_cost = gain, chosen, cost

    by_id = {candidate["action_id"]: candidate for candidate in candidates}
    selected = [by_id[action_id] for action_id in best_chosen]
    selected.sort(key=lambda item: item["action_id"])
    return best_cost, best_gain, selected


# --------------------------------------------------------------------------- #
# provenance and assembly
# --------------------------------------------------------------------------- #
def hash_inputs(input_dir):
    digests = {}
    for root, dirnames, filenames in os.walk(input_dir):
        dirnames.sort()
        for filename in sorted(filenames):
            path = os.path.join(root, filename)
            if not os.path.isfile(path):
                continue
            relative = os.path.relpath(path, input_dir).replace(os.sep, "/")
            digest = hashlib.sha256()
            with open(path, "rb") as handle:
                for chunk in iter(lambda: handle.read(1 << 20), b""):
                    digest.update(chunk)
            digests[relative] = digest.hexdigest()
    return digests


def build_results(input_dir):
    normalized_rows = load_normalized(input_dir)
    policy = load_policy(input_dir)
    event_records = load_event_records(input_dir)

    audit, effective_rows = apply_events(normalized_rows, event_records)
    aggregates = build_aggregates(normalized_rows, effective_rows)
    summary = build_summary(normalized_rows, effective_rows, audit)
    reconciliation = build_reconciliation(input_dir, normalized_rows)
    candidates = build_candidates(effective_rows, policy)

    budget = int(policy.get("budget_units", 0))
    spent, gain, selected = select_actions(candidates, max(budget, 0))
    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"] for row in effective_rows if row["active"]
    )

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
            "budget_units": budget,
            "spent_units": spent,
            "gain_milli": gain,
            "selected_action_ids": [item["action_id"] for item in selected],
            "projected_contribution_milli": current_contribution + gain,
        },
        "input_sha256": hash_inputs(input_dir),
    }
    return results


def main(argv=None):
    parser = argparse.ArgumentParser(description="107-v1 order audit pipeline")
    parser.add_argument("--input", default="input", help="input directory")
    parser.add_argument("--output", default="output", help="output directory")
    args = parser.parse_args(argv)

    results = build_results(os.path.abspath(args.input))

    output_dir = os.path.abspath(args.output)
    os.makedirs(output_dir, exist_ok=True)
    payload = json.dumps(results, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    with open(os.path.join(output_dir, "results.json"), "wb") as handle:
        handle.write(payload.encode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
