#!/usr/bin/env python3
"""107-V1 order audit, after-sales event ledger and budget optimisation pipeline.

Standard library only.  Usage:

    python pipeline.py --input input --output output

The program reads the eleven input files, normalises every CSV record to exact
integer amounts (milli currency units / ppm discount), applies the de-duplicated
after-sales event ledger, recomputes the latest operating metrics, audits the two
reference summary tables and solves the exact budget-constrained action
selection problem.  Nothing but ``<output>/results.json`` is written.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
from array import array
from datetime import date
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP

SCHEMA_VERSION = "107-v1"

MILLI_SCALE = 1000
PPM_SCALE = 1000000

# 通用指标中与政策无关的固定折扣阈值（严格大于）。
HIGH_DISCOUNT_PPM = 200000

STANDARD_SHIP_MODE = "Standard Class"
EVENT_TYPES = ("cancel", "refund", "shipping_credit")

# 聚合分组定义：(输出键, 分组字段元组)
GROUPINGS = (
    ("market", ("market",)),
    ("market_category", ("market", "category")),
    ("segment", ("segment",)),
    ("ship_mode", ("ship_mode",)),
    ("market_month", ("market", "month")),
)

ORDER_COLUMNS = {
    "order_id": "Order ID",
    "order_date": "Order Date",
    "ship_date": "Ship Date",
    "ship_mode": "Ship Mode",
    "segment": "Segment",
    "product_id": "Product ID",
    "sales": "Sales",
    "quantity": "Quantity",
    "discount": "Discount",
    "profit": "Profit",
    "shipping": "Shipping Cost",
    "priority": "Order Priority",
}

ORDER_SUFFIX = "_orders.csv"


# --------------------------------------------------------------------------- #
# 精确数值工具
# --------------------------------------------------------------------------- #
def round_half_up_div(numerator: int, denominator: int) -> int:
    """ROUND_HALF_UP 整数除法（远离零）。"""
    if denominator == 0:
        raise ZeroDivisionError("division by zero")
    negative = (numerator < 0) != (denominator < 0)
    num, den = abs(numerator), abs(denominator)
    quotient, remainder = divmod(num, den)
    if 2 * remainder >= den:
        quotient += 1
    return -quotient if negative else quotient


def fixed_from_int(value: int, places: int) -> str:
    """把已缩放的整数格式化为固定小数位的字符串。"""
    sign = "-" if value < 0 else ""
    magnitude = abs(value)
    if places == 0:
        return sign + str(magnitude)
    unit = 10 ** places
    integral, fraction = divmod(magnitude, unit)
    return "%s%d.%0*d" % (sign, integral, places, fraction)


def format_fixed(numerator: int, denominator: int, places: int) -> str:
    """把精确分数 numerator/denominator 按 places 位小数 ROUND_HALF_UP 输出。"""
    if denominator == 0:
        raise ZeroDivisionError("division by zero")
    scaled = round_half_up_div(numerator * (10 ** places), denominator)
    return fixed_from_int(scaled, places)


def ratio_bp(numerator: int, denominator: int):
    """基点整数；分母为零返回 None。"""
    if denominator == 0:
        return None
    return round_half_up_div(numerator * 10000, denominator)


def parse_scaled_int(text: str, scale: int) -> int:
    """把至多三位/六位小数的十进制字符串精确转换为缩放整数。"""
    raw = (text or "").strip()
    if raw == "":
        return 0
    value = Decimal(raw)
    return int((value * scale).to_integral_value(rounding=ROUND_HALF_UP))


def parse_plain_int(text: str) -> int:
    raw = (text or "").strip()
    if raw == "":
        return 0
    return int(Decimal(raw))


def quote_fixed(text: str, places: int) -> str:
    """把参考表里的十进制字符串按固定小数位规范化；无法解析时原样保留。"""
    raw = (text or "").strip()
    if raw == "":
        return ""
    try:
        value = Decimal(raw) * (10 ** places)
    except (InvalidOperation, ValueError):
        return raw
    return fixed_from_int(int(value.to_integral_value(rounding=ROUND_HALF_UP)), places)


# --------------------------------------------------------------------------- #
# 输入读取
# --------------------------------------------------------------------------- #
def read_csv_records(path: str):
    """读取 CSV，返回 (header, records)；空行不计为记录。"""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.reader(handle)
        header = None
        records = []
        for raw in reader:
            if not raw or all((cell or "").strip() == "" for cell in raw):
                continue
            if header is None:
                header = [(cell or "").strip().lstrip("\ufeff") for cell in raw]
                continue
            records.append(raw)
    return header or [], records


def cell(record, header, column) -> str:
    try:
        index = header.index(column)
    except ValueError:
        return ""
    if index >= len(record):
        return ""
    return (record[index] or "").strip()


def load_product_catalog(input_dir: str):
    path = os.path.join(input_dir, "product_info.csv")
    header, records = read_csv_records(path)
    catalog = {}
    for record in records:
        product_id = cell(record, header, "Product ID")
        if product_id not in catalog:
            catalog[product_id] = (
                cell(record, header, "Category"),
                cell(record, header, "Sub-Category"),
            )
    return catalog


def list_order_files(input_dir: str):
    names = [
        name
        for name in os.listdir(input_dir)
        if name.endswith(ORDER_SUFFIX)
        and os.path.isfile(os.path.join(input_dir, name))
    ]
    names.sort()
    return names


def load_normalized_rows(input_dir: str, catalog):
    rows = []
    for filename in list_order_files(input_dir):
        market = filename[: -len(ORDER_SUFFIX)]
        header, records = read_csv_records(os.path.join(input_dir, filename))
        sequence = 0
        for record in records:
            sequence += 1
            product_id = cell(record, header, ORDER_COLUMNS["product_id"])
            category, sub_category = catalog.get(product_id, ("", ""))
            order_date = cell(record, header, ORDER_COLUMNS["order_date"])
            ship_date = cell(record, header, ORDER_COLUMNS["ship_date"])
            rows.append(
                {
                    "row_key": "%s:%06d" % (filename, sequence),
                    "source_file": filename,
                    "source_row": sequence,
                    "order_id": cell(record, header, ORDER_COLUMNS["order_id"]),
                    "product_id": product_id,
                    "market": market,
                    "category": category,
                    "sub_category": sub_category,
                    "segment": cell(record, header, ORDER_COLUMNS["segment"]),
                    "ship_mode": cell(record, header, ORDER_COLUMNS["ship_mode"]),
                    "priority": cell(record, header, ORDER_COLUMNS["priority"]),
                    "order_date": order_date,
                    "ship_date": ship_date,
                    "delivery_days": (
                        _iso_date(ship_date) - _iso_date(order_date)
                    ).days,
                    "quantity": parse_plain_int(cell(record, header, ORDER_COLUMNS["quantity"])),
                    "sales_milli": parse_scaled_int(
                        cell(record, header, ORDER_COLUMNS["sales"]), MILLI_SCALE
                    ),
                    "profit_milli": parse_scaled_int(
                        cell(record, header, ORDER_COLUMNS["profit"]), MILLI_SCALE
                    ),
                    "shipping_milli": parse_scaled_int(
                        cell(record, header, ORDER_COLUMNS["shipping"]), MILLI_SCALE
                    ),
                    "discount_ppm": parse_scaled_int(
                        cell(record, header, ORDER_COLUMNS["discount"]), PPM_SCALE
                    ),
                }
            )
    rows.sort(key=lambda row: row["row_key"])
    return rows


def _iso_date(text: str):
    return date.fromisoformat((text or "").strip())


def load_events(path: str):
    """返回按输入顺序编号的非空事件记录 [(input_line, event_dict), ...]。"""
    entries = []
    with open(path, "r", encoding="utf-8-sig") as handle:
        for raw_line in handle:
            stripped = raw_line.strip()
            if not stripped:
                continue
            entries.append((len(entries) + 1, json.loads(stripped)))
    return entries


def load_policy(path: str):
    with open(path, "r", encoding="utf-8-sig") as handle:
        return json.load(handle)


# --------------------------------------------------------------------------- #
# 事件账本
# --------------------------------------------------------------------------- #
def is_json_int(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def apply_events(entries, effective_rows):
    """去重、排序并执行事件；返回 (event_audit, 状态计数)。"""
    index = {row["row_key"]: row for row in effective_rows}
    statuses = {}

    first_seen = set()
    kept = []
    for input_line, event in entries:
        event_id = event.get("event_id")
        if event_id in first_seen:
            statuses[input_line] = "duplicate_event"
        else:
            first_seen.add(event_id)
            kept.append((input_line, event))

    kept.sort(key=lambda item: (str(item[1].get("timestamp", "")), str(item[1].get("event_id", ""))))

    for input_line, event in kept:
        row_key = event.get("row_key")
        event_type = event.get("type")
        amount = event.get("amount_milli")

        row = index.get(row_key) if isinstance(row_key, str) else None
        if row is None:
            statuses[input_line] = "unknown_row"
            continue
        if event_type not in EVENT_TYPES:
            statuses[input_line] = "unknown_type"
            continue
        if not row["active"]:
            statuses[input_line] = "order_cancelled"
            continue

        if event_type == "cancel":
            valid = is_json_int(amount) and amount == 0
        elif event_type == "refund":
            valid = is_json_int(amount) and amount > 0 and amount <= row["sales_milli"]
        else:  # shipping_credit
            valid = is_json_int(amount) and amount > 0 and amount <= row["shipping_milli"]

        if not valid:
            statuses[input_line] = "invalid_amount"
            continue

        statuses[input_line] = "applied"
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

    audit = []
    for input_line, event in entries:
        audit.append(
            {
                "input_line": input_line,
                "event_id": event.get("event_id"),
                "row_key": event.get("row_key"),
                "status": statuses[input_line],
            }
        )

    counts = {}
    for record in audit:
        counts[record["status"]] = counts.get(record["status"], 0) + 1
    return audit, counts


# --------------------------------------------------------------------------- #
# 指标
# --------------------------------------------------------------------------- #
def compute_metrics(rows):
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
        "row_count": len(rows),
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


def group_key(row, fields):
    return tuple(row["order_date"][:7] if field == "month" else row[field] for field in fields)


def build_aggregates(normalized_rows, effective_rows):
    active = [row for row in effective_rows if row["active"]]
    result = {}
    for name, fields in GROUPINGS:
        buckets = {}
        for row in normalized_rows:
            buckets.setdefault(group_key(row, fields), [])
        for row in active:
            buckets.setdefault(group_key(row, fields), []).append(row)

        items = []
        for key in sorted(buckets):
            entry = {field: value for field, value in zip(fields, key)}
            entry.update(compute_metrics(buckets[key]))
            items.append(entry)
        result[name] = items
    return result


# --------------------------------------------------------------------------- #
# 参考表对账
# --------------------------------------------------------------------------- #
def segment_actuals(rows, fields):
    count = len(rows)
    sales = sum(row["sales_milli"] for row in rows)
    profit = sum(row["profit_milli"] for row in rows)
    discount = sum(row["discount_ppm"] for row in rows)

    actual = {}
    if "Order_Count" in fields:
        actual["Order_Count"] = fixed_from_int(count, 0)
    if "Total_Sales" in fields:
        actual["Total_Sales"] = format_fixed(sales, MILLI_SCALE, 2) if count else fixed_from_int(0, 2)
    if "Total_Profit" in fields:
        actual["Total_Profit"] = format_fixed(profit, MILLI_SCALE, 2) if count else fixed_from_int(0, 2)
    if "Profit_Margin" in fields:
        actual["Profit_Margin"] = (
            format_fixed(profit * 100, sales, 2) if sales else fixed_from_int(0, 2)
        )
    if "Avg_Discount" in fields:
        actual["Avg_Discount"] = (
            format_fixed(discount, count * PPM_SCALE, 4) if count else fixed_from_int(0, 4)
        )
    return actual


def shipping_actuals(rows, fields):
    count = len(rows)
    shipping = sum(row["shipping_milli"] for row in rows)
    days = sum(row["delivery_days"] for row in rows)

    actual = {}
    if "Order_Count" in fields:
        actual["Order_Count"] = fixed_from_int(count, 0)
    if "Total_Shipping_Cost" in fields:
        actual["Total_Shipping_Cost"] = (
            format_fixed(shipping, MILLI_SCALE, 2) if count else fixed_from_int(0, 2)
        )
    if "Avg_Shipping_Cost" in fields:
        actual["Avg_Shipping_Cost"] = (
            format_fixed(shipping, count * MILLI_SCALE, 2) if count else fixed_from_int(0, 2)
        )
    if "Avg_Delivery_Days" in fields:
        actual["Avg_Delivery_Days"] = (
            format_fixed(days, count, 2) if count else fixed_from_int(0, 2)
        )
    return actual


RECONCILIATION_SPECS = (
    {
        "source_file": "segment_performance.csv",
        "group_column": "Segment",
        "fields": {
            "Order_Count": 0,
            "Total_Sales": 2,
            "Total_Profit": 2,
            "Profit_Margin": 2,
            "Avg_Discount": 4,
        },
        "compute": segment_actuals,
    },
    {
        "source_file": "shipping_by_mode.csv",
        "group_column": "Ship_Mode",
        "fields": {
            "Order_Count": 0,
            "Total_Shipping_Cost": 2,
            "Avg_Shipping_Cost": 2,
            "Avg_Delivery_Days": 2,
        },
        "compute": shipping_actuals,
    },
)


def build_reconciliation(input_dir, normalized_rows):
    records = []
    for spec in RECONCILIATION_SPECS:
        path = os.path.join(input_dir, spec["source_file"])
        if not os.path.isfile(path):
            continue
        header, reference_rows = read_csv_records(path)
        group_column = spec["group_column"]
        field_names = sorted(spec["fields"])
        present = [name for name in field_names if name in header]

        bucketed = {}
        for row in normalized_rows:
            key = row["segment"] if group_column == "Segment" else row["ship_mode"]
            bucketed.setdefault(key, []).append(row)

        entries = []
        for record in reference_rows:
            group = cell(record, header, group_column)
            actuals = spec["compute"](bucketed.get(group, []), present)
            for field in present:
                actual = actuals[field]
                reference = quote_fixed(
                    cell(record, header, field), spec["fields"][field]
                )
                entries.append(
                    {
                        "source_file": spec["source_file"],
                        "group": group,
                        "field": field,
                        "actual": actual,
                        "reference": reference,
                        "match": actual == reference,
                    }
                )

        entries.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
        records.extend(entries)

    records.sort(key=lambda item: (item["source_file"], item["group"], item["field"]))
    return records


# --------------------------------------------------------------------------- #
# 候选动作与预算最优化
# --------------------------------------------------------------------------- #
def build_action_candidates(effective_rows, policy):
    priorities = policy.get("shipping_priorities") or []
    shipping_saving_bp = int(policy.get("shipping_saving_bp", 0))
    discount_threshold_ppm = int(policy.get("discount_threshold_ppm", 0))
    discount_recovery_bp = int(policy.get("discount_recovery_bp", 0))
    shipping_costs = policy.get("shipping_cost_by_market") or {}
    discount_costs = policy.get("discount_cost_by_market") or {}

    candidates = []
    for row in effective_rows:
        row_key = row["row_key"]
        market = row["market"]

        if row["ship_mode"] != STANDARD_SHIP_MODE and row["priority"] in priorities:
            gain = row["shipping_milli"] * shipping_saving_bp // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row_key + "|shipping_switch",
                        "row_key": row_key,
                        "type": "shipping_switch",
                        "cost_units": int(shipping_costs.get(market, 0)),
                        "gain_milli": gain,
                    }
                )

        contribution = row["profit_milli"] - row["shipping_milli"]
        if row["discount_ppm"] > discount_threshold_ppm and contribution < 0:
            gain = row["sales_milli"] * discount_recovery_bp // 10000
            if gain > 0:
                candidates.append(
                    {
                        "action_id": row_key + "|discount_review",
                        "row_key": row_key,
                        "type": "discount_review",
                        "cost_units": int(discount_costs.get(market, 0)),
                        "gain_milli": gain,
                    }
                )

    candidates.sort(key=lambda item: item["action_id"])
    return candidates


NEG = -1


def advance_layer(next_layer, group, capacity):
    """由 rows[i+1..] 的后缀 DP 层推出 rows[i..] 的后缀 DP 层。"""
    current = array("q", [NEG]) * (capacity + 1)
    options = [(item["cost_units"], item["gain_milli"]) for item in group]
    for cost in range(capacity + 1):
        best = next_layer[cost]
        for item_cost, item_gain in options:
            if item_cost <= cost:
                previous = next_layer[cost - item_cost]
                if previous >= 0:
                    value = previous + item_gain
                    if value > best:
                        best = value
        current[cost] = best
    return current


def select_actions(candidates, budget_units):
    """多重选择 0/1 背包：先最大化收益，再最小化成本，再取字典序最小的方案。"""
    if not candidates:
        return [], 0, 0

    by_row = {}
    for candidate in candidates:  # 已按 action_id 升序
        by_row.setdefault(candidate["row_key"], []).append(candidate)

    row_keys = sorted(by_row)
    options = [by_row[key] for key in row_keys]

    max_total_cost = sum(max(item["cost_units"] for item in group) for group in options)
    budget = max(0, int(budget_units))

    if budget >= max_total_cost:
        chosen = []
        for group in options:
            best = max(group, key=lambda item: (item["gain_milli"], -item["cost_units"]))
            chosen.append(best)
        chosen.sort(key=lambda item: item["action_id"])
        return chosen, sum(item["cost_units"] for item in chosen), sum(
            item["gain_milli"] for item in chosen
        )

    capacity = budget
    row_total = len(options)
    block = max(1, int(row_total ** 0.5))

    # 后缀 DP：dp[i][c] = 只用 rows[i..] 且成本恰为 c 时的最大收益。
    # 只保留每 block 行的检查点，重建时按块重算，内存约为 (n/block + block) * capacity。
    checkpoints = {}
    nxt = array("q", [NEG]) * (capacity + 1)
    nxt[0] = 0
    checkpoints[row_total] = nxt
    for index in range(row_total - 1, -1, -1):
        nxt = advance_layer(nxt, options[index], capacity)
        if index % block == 0:
            checkpoints[index] = nxt

    first_layer = checkpoints[0]
    best_gain = max(first_layer[cost] for cost in range(capacity + 1))
    if best_gain <= 0:
        return [], 0, 0
    best_cost = min(
        cost for cost in range(capacity + 1) if first_layer[cost] == best_gain
    )

    chosen = []
    remaining = best_cost
    index = 0
    while index < row_total:
        block_start = (index // block) * block
        block_end = min(block_start + block, row_total)
        # 后缀层只能由后向前推出：从 block_end 的检查点回推到 block_start。
        layers = [None] * (block_end - block_start + 1)
        layers[block_end - block_start] = checkpoints[block_end]
        for offset in range(block_end - block_start, 0, -1):
            layers[offset - 1] = advance_layer(
                layers[offset], options[block_start + offset - 1], capacity
            )

        while index < block_end:
            slot = index - block_start
            target = layers[slot][remaining]
            for item in options[index]:
                item_cost = item["cost_units"]
                if item_cost > remaining:
                    continue
                previous = layers[slot + 1][remaining - item_cost]
                if previous >= 0 and item["gain_milli"] + previous == target:
                    chosen.append(item)
                    remaining -= item_cost
                    break
            index += 1

    chosen.sort(key=lambda item: item["action_id"])
    return chosen, sum(item["cost_units"] for item in chosen), sum(
        item["gain_milli"] for item in chosen
    )


# --------------------------------------------------------------------------- #
# 溯源
# --------------------------------------------------------------------------- #
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


# --------------------------------------------------------------------------- #
# 主流程
# --------------------------------------------------------------------------- #
def build_results(input_dir):
    catalog = load_product_catalog(input_dir)
    normalized_rows = load_normalized_rows(input_dir, catalog)

    effective_rows = []
    for row in normalized_rows:
        copy = dict(row)
        copy["active"] = True
        effective_rows.append(copy)

    entries = load_events(os.path.join(input_dir, "adjustments.jsonl"))
    event_audit, status_counts = apply_events(entries, effective_rows)

    aggregates = build_aggregates(normalized_rows, effective_rows)
    reconciliation = build_reconciliation(input_dir, normalized_rows)

    active_rows = [row for row in effective_rows if row["active"]]
    summary = compute_metrics(active_rows)
    unique_orders = len({row["order_id"] for row in normalized_rows})
    summary["input_row_count"] = len(normalized_rows)
    summary["unique_order_id_count"] = unique_orders
    summary["duplicate_order_id_extra_rows"] = len(normalized_rows) - unique_orders
    summary["cancelled_row_count"] = len(normalized_rows) - len(active_rows)
    summary["event_status_counts"] = {
        status: status_counts[status] for status in sorted(status_counts)
    }

    policy = load_policy(os.path.join(input_dir, "policy.json"))
    candidates = build_action_candidates(effective_rows, policy)
    selected, spent, gain = select_actions(candidates, policy.get("budget_units", 0))

    current_contribution = sum(
        row["profit_milli"] - row["shipping_milli"] for row in active_rows
    )
    optimization = {
        "budget_units": int(policy.get("budget_units", 0)),
        "spent_units": spent,
        "gain_milli": gain,
        "selected_action_ids": [item["action_id"] for item in selected],
        "projected_contribution_milli": current_contribution + gain,
    }

    return {
        "schema_version": SCHEMA_VERSION,
        "normalized_rows": normalized_rows,
        "event_audit": event_audit,
        "effective_rows": effective_rows,
        "aggregates": aggregates,
        "reconciliation": reconciliation,
        "summary": summary,
        "action_candidates": candidates,
        "selected_actions": selected,
        "optimization": optimization,
        "input_sha256": hash_inputs(input_dir),
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="107-V1 order audit / event ledger / budget optimisation pipeline"
    )
    parser.add_argument("--input", default="input", help="input directory")
    parser.add_argument("--output", default="output", help="output directory")
    args = parser.parse_args(argv)

    results = build_results(args.input)

    os.makedirs(args.output, exist_ok=True)
    target = os.path.join(args.output, "results.json")
    text = json.dumps(results, ensure_ascii=False, indent=2) + "\n"
    with open(target, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
