"""Parse the observed installed NCU raw-page wide CSV; export only eight exact metrics."""
import csv
import io
import math
from aster_ncu_metrics_v171 import METRICS


def numeric_wide_rows(text):
    if type(text) is not str or len(text.encode()) > 2 * 1024**2:
        raise ValueError('bounded wide profiler CSV exceeded')
    header = None
    units = None
    rows = []
    seen = set()
    for fields in csv.reader(io.StringIO(text)):
        if header is None:
            if {'ID', 'Kernel Name', *METRICS} <= set(fields):
                if len(fields) != len(set(fields)) or len(fields) > 2048:
                    raise ValueError('duplicate or excessive wide profiler columns')
                header = fields
            continue
        if not fields or all(not value for value in fields):
            continue
        if len(fields) != len(header):
            raise ValueError('wide profiler row width differs')
        row = dict(zip(header, fields))
        if units is None:
            if row['ID'] or row['Kernel Name'] or any(not 0 < len(row[name]) <= 128 for name in METRICS):
                raise ValueError('exact wide profiler units row absent')
            units = {name: row[name] for name in METRICS}
            continue
        identifier = row['ID']
        if not identifier.isascii() or not identifier.isdecimal() or int(identifier) > 100000 or identifier in seen:
            raise ValueError('duplicate or invalid wide profiler kernel ID')
        seen.add(identifier)
        symbol = row['Kernel Name']
        if not 0 < len(symbol) <= 8192:
            raise ValueError('bounded wide profiler kernel symbol exceeded')
        for name in METRICS:
            value = float(row[name].replace(',', ''))
            if not math.isfinite(value) or value < 0:
                raise ValueError('wide profiler metric must be finite and nonnegative')
            rows.append(dict(kernel_id=int(identifier), symbol=symbol, metric=name, unit=units[name], value=value))
        if len(rows) > 4096:
            raise ValueError('bounded wide profiler numeric row count exceeded')
    if header is None or units is None or not rows:
        raise ValueError('wide profiler header/units/data absent')
    return rows
