#!/usr/bin/env python3
"""107-V1: order audit, event ledger, metrics, reconciliation and budget optimization.

Standard library only. Run as:

    python pipeline.py --input input --output output
"""

import argparse
import csv
import hashlib
import json
import os
from decimal import Decimal, ROUND_HALF_UP, localcontext

SCHEMA_VERSION = "107-v1"
VALID_EVENT_TYPES = ("cancel", "refund", "shipping_credit")
ORDERS_SUFFIX = "_orders.csv"

# ---------------------------------------------------------------- arithmetic


def round_half_up_div(num, den):
    """Round num/den to an integer using ROUND_HALF_UP (ties away from zero)."""
    if den == 0:
        raise ZeroDivisionError("division by zero")
    if den < 0:
        num, den = -num, -den
    negative = num < 0
    n = -num if negative else num
    quotient, remainder = divmod(n, den)
    if 2 * remainder >= den:
        quotient += 1
    return -quotient if negative else quotient


def fixed_str(scaled, decimals):
    """Render an integer scaled by 10**decimals as a fixed-point string."""
    negative = scaled < 0
    n = -scaled if negative else scaled
    if decimals <= 0:
        body = str(n)
    else:
        unit = 10 ** decimals
        whole, frac = divmod(n, unit)
        body = "%d.%0*d" % (whole, decimals, frac)
    return "-" + body if negative else body


def fmt_ratio(num, den, decimals):
    """Exact num/den rounded once, half up, to `decimals` places as a string."""
    if den == 0:
        return fixed_str(0, decimals)
    return fixed_str(round_half_up_div(num * (10 ** decimals), den), decimals)


def parse_scaled(text, scale):
    """Convert a decimal string to an integer scaled by `scale` exactly."""
    s = "" if text is None else str(text).strip()
    if s == "":
        return 0
    with localcontext() as ctx:
        ctx.prec = 60
        value = Decimal(s) * scale
        return int(value.to_integral_value(rounding=ROUND_HALF_UP))


def parse_int(text):
    s = "" if text is None else str(text).strip()
    if s == "":
        return 0
    return int(s)


def quantize_str(text, decimals):
    """Normalise a reference-table value to a fixed number of decimals."""
    s = "" if text is None else str(text).strip()
    with localcontext() as ctx:
        ctx.prec = 60
        value = Decimal(s) if s != "" else Decimal(0)
        quantum = Decimal(1).scaleb(-decimals)
        out = value.quantize(quantum, rounding=ROUND_HALF_UP)
    if out == 0:
        out = abs(out)
    return format(out, "f")


def is_plain_int(value):
    return isinstance(value, int) and not isinstance(value, bool)


# ------------------------------------------------------------------- file io


def read_csv_table(path):
    """Return (header, data_rows) skipping blank lines."""
    header = None
    rows = []
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        for raw in csv.reader(handle):
            if not any((cell or "").strip() for cell in raw):
                continue
            if header is None:
                header = [cell.strip() for cell in raw]
            else:
                rows.append(raw)
    return header or [], rows


def cell(row, index):
    if index is None or index < 0 or index >= len(row):
        return ""
    return row[index]


def index_map(header):
    return {name: pos for pos, name in enumerate(header)}


def load_product_catalog(input_dir):
    path = os.path.join(input_dir, "product_info.csv")
    catalog = {}
    if not os.path.exists(path):
        return catalog
    header, rows = read_csv_table(path)
    idx = index_map(header)
    for row in rows:
        pid = cell(row, idx.get("Product ID")).strip()
        if pid and pid not in catalog:
            catalog[pid] = (
                cell(row, idx.get("Category")).strip(),
                cell(row, idx.get("Sub-Category")).strip(),
            )
    return catalog


def order_files(input_dir):
    names = []
    for name in os.listdir(input_dir):
        full = os.path.join(input_dir, name)
        if os.path.isfile(full) and name.endswith(ORDERS_SUFFIX):
            names.append(name)
    return sorted(names)


def build_normalized_rows(input_dir):
    catalog = load_product_catalog(input_dir)
    rows = []
    for name in order_files(input_dir):
        market = name[: -len(ORDERS_SUFFIX)]
        header, data = read_csv_table(os.path.join(input_dir, name))
        idx = index_map(header)
        seq = 0
        for raw in data:
            seq += 1
            product_id = cell(raw, idx.get("Product ID")).strip()
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date = cell(raw, idx.get("Order Date")).strip()
            ship_date = cell(raw, idx.get("Ship Date")).strip()
            rows.append(
                {
                    "row_key": "%s:%06d" % (name, seq),
                    "source_file": name,
                    "source_row": seq,
                    "order_id": cell(raw, idx.get("Order ID")).strip(),
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": cell(raw, idx.get("Segment")).strip(),
                    "ship_mode": cell(raw, idx.get("Ship Mode")).strip(),
                    "priority": cell(raw, idx.get("Order Priority")).strip(),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": delivery_days(order_date, ship_date),
                    "quantity": parse_int(cell(raw, idx.get("Quantity"))),
                    "sales_milli": parse_scaled(cell(raw, idx.get("Sales")), 1000),
                    "profit_milli": parse_scaled(cell(raw, idx.get("Profit")), 1000),
                    "shipping_milli": parse_scaled(
                        cell(raw, idx.get("Shipping Cost")), 1000
                    ),
                    "discount_ppm": parse_scaled(cell(raw, idx.get("Discount")), 1000000),
                }
            )
    rows.sort(key=lambda item: item["row_key"])
    return rows


def delivery_days(order_date, ship_date):
    from datetime import date

    try:
        start = date.fromisoformat(order_date)
        end = date.fromisoformat(ship_date)
    except ValueError:
        return 0
    return (end - start).days


# ------------------------------------------------------------------- events


def load_events(path):
    records = []
    if not os.path.exists(path):
        return records
    with open(path, "rb") as handle:
        text = handle.read().decode("utf-8-sig")
    for line in text.splitlines():
        if not line.strip():
            continue
        records.append(json.loads(line))
    return records


def apply_events(normalized_rows, event_records):
    """Return (audit_entries, state_by_row_key)."""
    state = {}
    for row in normalized_rows:
        state[row["row_key"]] = {
            "active": True,
            "sales_milli": row["sales_milli"],
            "profit_milli": row["profit_milli"],
            "shipping_milli": row["shipping_milli"],
        }

    seen = set()
    unique = []
    duplicate_flags = []
    for record in event_records:
        event_id = record.get("event_id")
        if event_id in seen:
            duplicate_flags.append(True)
        else:
            seen.add(event_id)
            duplicate_flags.append(False)
            unique.append(record)

    ordered = sorted(
        unique,
        key=lambda rec: (str(rec.get("timestamp", "")), str(rec.get("event_id", ""))),
    )

    status_by_id = {}
    for record in ordered:
        event_id = record.get("event_id")
        row_key = record.get("row_key")
        event_type = record.get("type")
        amount = record.get("amount_milli")
        target = state.get(row_key) if isinstance(row_key, str) else None

        if target is None:
            status = "unknown_row"
        elif event_type not in VALID_EVENT_TYPES:
            status = "unknown_type"
        elif not target["active"]:
            status = "order_cancelled"
        else:
            valid = is_plain_int(amount)
            if event_type == "cancel":
                valid = valid and amount == 0
            elif event_type == "refund":
                valid = valid and amount > 0 and amount <= target["sales_milli"]
            else:  # shipping_credit
                valid = valid and amount > 0 and amount <= target["shipping_milli"]

            if not valid:
                status = "invalid_amount"
            else:
                status = "applied"
                if event_type == "cancel":
                    target["active"] = False
                    target["sales_milli"] = 0
                    target["profit_milli"] = 0
                    target["shipping_milli"] = 0
                elif event_type == "refund":
                    target["sales_milli"] -= amount
                    target["profit_milli"] -= amount
                else:
                    target["shipping_milli"] -= amount
        status_by_id[event_id] = status

    audit = []
    for position, record in enumerate(event_records, start=1):
        event_id = record.get("event_id")
        if duplicate_flags[position - 1]:
            status = "duplicate_event"
        else:
            status = status_by_id.get(event_id)
        audit.append(
            {
                "input_line": position,
                "event_id": event_id,
                "row_key": record.get("row_key"),
                "status": status,
            }
        )
    return audit, state


def build_effective_rows(normalized_rows, state):
    effective = []
    for row in normalized_rows:
        current = state[row["row_key"]]
        item = dict(row)
        item["sales_milli"] = current["sales_milli"]
        item["profit_milli"] = current["profit_milli"]
        item["shipping_milli"] = current["shipping_milli"]
        item["active"] = current["active"]
        effective.append(item)
    effective.sort(key=lambda item: item["row_key"])
    return effective


# ------------------------------------------------------------------ metrics


def common_metrics(rows):
    row_count = len(rows)
    quantity = sum(row["quantity"] for row in rows)
    sales = sum(row["sales_milli"] for row in rows)
    profit = sum(row["profit_milli"] for row in rows)
    shipping = sum(row["shipping_milli"] for row in rows)
    contribution = profit - shipping
    high_discount = sum(1 for row in rows if row["discount_ppm"] > 200000)
    negative_contribution = sum(
        1 for row in rows if row["profit_milli"] < row["shipping_milli"]
    )
    return {
        "row_count": row_count,
        "quantity": quantity,
        "sales_milli": sales,
        "profit_milli": profit,
        "shipping_milli": shipping,
        "contribution_milli": contribution,
        "profit_margin_bp": (
            None if sales == 0 else round_half_up_div(profit * 10000, sales)
        ),
        "contribution_margin_bp": (
            None if sales == 0 else round_half_up_div(contribution * 10000, sales)
        ),
        "high_discount_count": high_discount,
        "negative_contribution_count": negative_contribution,
    }


def build_aggregates(normalized_rows, effective_rows):
    active_rows = [row for row in effective_rows if row["active"]]

    def month_of(row):
        return row["order_date"][:7]

    group_specs = [
        ("market", ("market",)),
        ("market_category", ("market", "category")),
        ("segment", ("segment",)),
        ("ship_mode", ("ship_mode",)),
        ("market_month", ("market", "month")),
    ]

    aggregates = {}
    for list_name, fields in group_specs:
        def key_of(row, fields=fields):
            values = []
            for field in fields:
                values.append(month_of(row) if field == "month" else row[field])
            return tuple(values)

        keys = set()
        for row in normalized_rows:
            keys.add(key_of(row))

        buckets = {}
        for row in active_rows:
            buckets.setdefault(key_of(row), []).append(row)

        entries = []
        for key in sorted(keys):
            entry = {}
            for field, value in zip(fields, key):
                entry[field] = value
            entry.update(common_metrics(buckets.get(key, [])))
            entries.append(entry)
        aggregates[list_name] = entries
    return aggregates


def build_summary(normalized_rows, effective_rows, audit):
    active_rows = [row for row in effective_rows if row["active"]]
    summary = common_metrics(active_rows)
    order_ids = {row["order_id"] for row in normalized_rows}
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = len(order_ids)
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - len(order_ids)
    summary["cancelled_row_count"] = sum(
        1 for row in effective_rows if not row["active"]
    )
    counts = {}
    for entry in audit:
        counts[entry["status"]] = counts.get(entry["status"], 0) + 1
    summary["event_status_counts"] = counts
    return summary


# ----------------------------------------------------------- reconciliation

SEGMENT_FIELDS = [
    ("Order_Count", 0),
    ("Total_Sales", 2),
    ("Total_Profit", 2),
    ("Profit_Margin", 2),
    ("Avg_Discount", 4),
]
SHIPPING_FIELDS = [
    ("Order_Count", 0),
    ("Total_Shipping_Cost", 2),
    ("Avg_Shipping_Cost", 2),
    ("Avg_Delivery_Days", 2),
]


def segment_actual(field, rows):
    count = len(rows)
    if field == "Order_Count":
        return fixed_str(count, 0)
    if field == "Total_Sales":
        return fmt_ratio(sum(r["sales_milli"] for r in rows), 1000, 2)
    if field == "Total_Profit":
        return fmt_ratio(sum(r["profit_milli"] for r in rows), 1000, 2)
    if field == "Profit_Margin":
        profit = sum(r["profit_milli"] for r in rows)
        sales = sum(r["sales_milli"] for r in rows)
        return fmt_ratio(profit * 100, sales, 2)
    if field == "Avg_Discount":
        if count == 0:
            return fixed_str(0, 4)
        return fmt_ratio(sum(r["discount_ppm"] for r in rows), count * 1000000, 4)
    raise KeyError(field)


def shipping_actual(field, rows):
    count = len(rows)
    if field == "Order_Count":
        return fixed_str(count, 0)
    if field == "Total_Shipping_Cost":
        return fmt_ratio(sum(r["shipping_milli"] for r in rows), 1000, 2)
    if field == "Avg_Shipping_Cost":
        if count == 0:
            return fixed_str(0, 2)
        return fmt_ratio(sum(r["shipping_milli"] for r in rows), count * 1000, 2)
    if field == "Avg_Delivery_Days":
        if count == 0:
            return fixed_str(0, 2)
        return fmt_ratio(sum(r["delivery_days"] for r in rows), count, 2)
    raise KeyError(field)


def build_reconciliation(input_dir, normalized_rows):
    specs = [
        ("segment_performance.csv", "Segment", "segment", SEGMENT_FIELDS, segment_actual),
        ("shipping_by_mode.csv", "Ship_Mode", "ship_mode", SHIPPING_FIELDS, shipping_actual),
    ]
    results = []
    for filename, group_column, row_field, fields, actual_fn in specs:
        path = os.path.join(input_dir, filename)
        if not os.path.exists(path):
            continue
        header, rows = read_csv_table(path)
        idx = index_map(header)
        group_index = idx.get(group_column, 0)

        buckets = {}
        for row in normalized_rows:
            buckets.setdefault(row[row_field], []).append(row)

        for ref_row in rows:
            group = cell(ref_row, group_index).strip()
            group_rows = buckets.get(group, [])
            for field, decimals in fields:
                actual = actual_fn(field, group_rows)
                reference = quantize_str(cell(ref_row, idx.get(field)), decimals)
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


# -------------------------------------------------------------- optimization


def build_action_candidates(effective_rows, policy):
    shipping_priorities = set(policy.get("shipping_priorities", []) or [])
    saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold = int(policy.get("discount_threshold_ppm", 0))
    recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_cost = policy.get("shipping_cost_by_market", {}) or {}
    discount_cost = policy.get("discount_cost_by_market", {}) or {}

    candidates = []
    for row in effective_rows:
        if row["ship_mode"] != "Standard Class" and row["priority"] in shipping_priorities:
            gain = (row["shipping_milli"] * saving_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row["row_key"] + "|shipping_switch",
                        "row_key": row["row_key"],
                        "type": "shipping_switch",
                        "cost_units": int(shipping_cost[row["market"]]),
                        "gain_milli": gain,
                    }
                )
        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold and contribution < 0:
            gain = (row["sales_milli"] * recovery_bp) // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row["row_key"] + "|discount_review",
                        "row_key": row["row_key"],
                        "type": "discount_review",
                        "cost_units": int(discount_cost[row["market"]]),
                        "gain_milli": gain,
                    }
                )
    candidates.sort(key=lambda item: item["action_id"])
    return candidates


def _better(candidate, incumbent):
    """Prefer higher gain, then lexicographically smaller id list."""
    if candidate[0] != incumbent[0]:
        return candidate[0] > incumbent[0]
    return tuple(sorted(candidate[1])) < tuple(sorted(incumbent[1]))


def optimize(candidates, budget):
    by_row = {}
    for item in candidates:
        by_row.setdefault(item["row_key"], []).append(item)

    total_cost = sum(item["cost_units"] for item in candidates)
    cap = min(max(budget, 0), total_cost)

    dp = [None] * (cap + 1)
    dp[0] = (0, ())
    reachable = 0

    for row_key in sorted(by_row):
        options = by_row[row_key]
        next_dp = dp[:]
        new_reachable = reachable
        for cost in range(reachable + 1):
            state = dp[cost]
            if state is None:
                continue
            base_gain, base_ids = state
            for option in options:
                new_cost = cost + option["cost_units"]
                if new_cost > cap:
                    continue
                proposal = (base_gain + option["gain_milli"], base_ids + (option["action_id"],))
                current = next_dp[new_cost]
                if current is None or _better(proposal, current):
                    next_dp[new_cost] = proposal
                    if new_cost > new_reachable:
                        new_reachable = new_cost
        dp = next_dp
        reachable = new_reachable

    best = None  # (gain, cost, ids)
    for cost in range(cap + 1):
        state = dp[cost]
        if state is None:
            continue
        gain, ids = state
        if best is None:
            best = (gain, cost, ids)
            continue
        if gain > best[0]:
            best = (gain, cost, ids)
        elif gain == best[0]:
            if cost < best[1]:
                best = (gain, cost, ids)
            elif cost == best[1] and tuple(sorted(ids)) < tuple(sorted(best[2])):
                best = (gain, cost, ids)

    if best is None:
        best = (0, 0, ())

    selected_ids = set(best[2])
    selected = [item for item in candidates if item["action_id"] in selected_ids]
    selected.sort(key=lambda item: item["action_id"])
    return selected, best[0], best[1]


# -------------------------------------------------------------------- main


def sha256_map(input_dir):
    digests = {}
    for name in sorted(os.listdir(input_dir)):
        full = os.path.join(input_dir, name)
        if not os.path.isfile(full):
            continue
        hasher = hashlib.sha256()
        with open(full, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                hasher.update(chunk)
        digests[name] = hasher.hexdigest()
    return digests


def run(input_dir, output_dir):
    normalized_rows = build_normalized_rows(input_dir)

    with open(os.path.join(input_dir, "policy.json"), "r", encoding="utf-8-sig") as fh:
        policy = json.load(fh)

    event_records = load_events(os.path.join(input_dir, "adjustments.jsonl"))
    audit, state = apply_events(normalized_rows, event_records)
    effective_rows = build_effective_rows(normalized_rows, state)

    aggregates = build_aggregates(normalized_rows, effective_rows)
    summary = build_summary(normalized_rows, effective_rows, audit)
    reconciliation = build_reconciliation(input_dir, normalized_rows)

    candidates = build_action_candidates(effective_rows, policy)
    selected, gain, spent = optimize(candidates, int(policy.get("budget_units", 0)))

    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"]
        for row in effective_rows
        if row["active"]
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
            "budget_units": int(policy.get("budget_units", 0)),
            "spent_units": spent,
            "gain_milli": gain,
            "selected_action_ids": [item["action_id"] for item in selected],
            "projected_contribution_milli": current_contribution + gain,
        },
        "input_sha256": sha256_map(input_dir),
    }

    os.makedirs(output_dir, exist_ok=True)
    target = os.path.join(output_dir, "results.json")
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(results, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
    return target


def main():
    parser = argparse.ArgumentParser(description="107-V1 order analytics pipeline")
    parser.add_argument("--input", required=True, help="input directory")
    parser.add_argument("--output", required=True, help="output directory")
    args = parser.parse_args()
    run(args.input, args.output)


if __name__ == "__main__":
    main()
