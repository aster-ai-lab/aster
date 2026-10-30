"""Bounded parsing of the installed profiler's explicitly named numeric CSV columns."""
import csv
import io
import math

METRICS=(
    'gpu__time_duration.sum',
    'sm__throughput.avg.pct_of_peak_sustained_elapsed',
    'dram__throughput.avg.pct_of_peak_sustained_elapsed',
    'sm__warps_active.avg.pct_of_peak_sustained_active',
    'dram__bytes_read.sum',
    'dram__bytes_write.sum',
    'smsp__warps_eligible.avg.per_cycle_active',
    'smsp__issue_active.avg.pct_of_peak_sustained_active',
)
FAMILIES=('native_gu_multi_kernel','native_gu_amd_kernel','native_down_multi_kernel',
          'native_down_amd_kernel','mul_mat_vec_q','flash_attn_ext_f16')
_COLUMNS={'ID','Kernel Name','Metric Name','Metric Unit','Metric Value'}

def numeric_rows(text):
    if type(text) is not str or len(text.encode())>2*1024**2:
        raise ValueError('bounded profiler numeric CSV exceeded')
    reader=csv.reader(io.StringIO(text));header=None;rows=[];seen=set()
    for fields in reader:
        if header is None:
            if _COLUMNS<=set(fields):
                if len(fields)!=len(set(fields)):raise ValueError('duplicate profiler CSV header')
                header=fields
            continue
        if not fields or all(not value for value in fields):continue
        if len(fields)!=len(header):raise ValueError('profiler CSV row width differs')
        row=dict(zip(header,fields));metric=row['Metric Name']
        if metric not in METRICS:continue
        value=float(row['Metric Value'].replace(',',''))
        if not math.isfinite(value) or value<0:raise ValueError('profiler metric is not a finite nonnegative number')
        identifier=row['ID']
        if not identifier.isascii() or not identifier.isdecimal() or int(identifier)>100000:
            raise ValueError('invalid profiler kernel ID')
        symbol=row['Kernel Name'];unit=row['Metric Unit']
        if not 0<len(symbol)<=8192 or len(unit)>128:raise ValueError('bounded profiler symbol/unit exceeded')
        key=(identifier,metric)
        if key in seen:raise ValueError('duplicate profiler kernel metric')
        seen.add(key);rows.append(dict(kernel_id=int(identifier),symbol=symbol,metric=metric,unit=unit,value=value))
        if len(rows)>4096:raise ValueError('bounded profiler numeric row count exceeded')
    if header is None or not rows:raise ValueError('profiler numeric CSV header/data absent')
    missing=set(METRICS)-{row['metric'] for row in rows}
    if missing:raise ValueError('requested profiler metrics absent: '+','.join(sorted(missing)))
    return rows

def family_counts(rows):
    result={family:set() for family in FAMILIES}
    for row in rows:
        # Literal identifiers come from the completed Nsight Systems symbol list.
        # Matching only selects these declared CUDA functions; it infers no names.
        for family in FAMILIES:
            if family+'<' in row['symbol'] or family+'(' in row['symbol']:
                result[family].add(row['kernel_id'])
    return {name:len(values) for name,values in result.items()}
