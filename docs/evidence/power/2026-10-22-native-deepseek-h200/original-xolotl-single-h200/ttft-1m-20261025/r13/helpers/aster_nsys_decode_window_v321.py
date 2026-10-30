"""Read exact observed CUDA profiler boundaries from a private Nsight SQLite export."""

START_API = 'cudaProfilerStart_v4000'
STOP_API = 'cudaProfilerStop_v4000'


def decode_window(db, *, require_all_kernels=False):
    required = {
        'StringIds': {'id', 'value'},
        'CUPTI_ACTIVITY_KIND_RUNTIME': {'start', 'end', 'globalTid', 'nameId', 'returnValue'},
        'CUPTI_ACTIVITY_KIND_KERNEL': {'start', 'end', 'demangledName'},
        'GPU_METRICS': {'timestamp', 'typeId', 'metricId', 'value'},
    }
    for table, columns in required.items():
        actual = {r[1] for r in db.execute('PRAGMA table_info("' + table + '")')}
        if not columns <= actual:
            raise ValueError('required observed trace schema differs: ' + table)
    rows = list(db.execute(
        'SELECT s.value,r.start,r.end,r.globalTid,r.returnValue '
        'FROM CUPTI_ACTIVITY_KIND_RUNTIME r JOIN StringIds s ON s.id=r.nameId '
        'WHERE s.value IN (?,?) ORDER BY r.start', (START_API, STOP_API)))
    if len(rows) != 2 or [r[0] for r in rows] != [START_API, STOP_API]:
        raise ValueError('exactly one ordered profiler start/stop pair required')
    for row in rows:
        if any(type(x) is not int for x in row[1:]) or row[1] < 0 or row[2] < row[1] or row[4] != 0:
            raise ValueError('successful integral profiler API boundaries required')
    start, stop = rows
    if start[3] != stop[3] or start[2] >= stop[1]:
        raise ValueError('same-thread nonoverlapping profiler boundaries required')
    lo, hi = start[2], stop[1]
    total, inside, overlap = db.execute(
        'SELECT COUNT(*),SUM(start>=? AND end<=?),SUM(start<? AND end>?) '
        'FROM CUPTI_ACTIVITY_KIND_KERNEL', (lo, hi, hi, lo)).fetchone()
    if not inside or overlap != inside:
        raise ValueError('nonempty kernel window with no boundary-straddling kernels required')
    if require_all_kernels and inside != total:
        raise ValueError('smoke kernels outside the profiler API window')
    kernels = [dict(name=name, calls=count, summed_ns=duration) for name, count, duration in db.execute(
        'SELECT s.value,COUNT(*),SUM(k.end-k.start) FROM CUPTI_ACTIVITY_KIND_KERNEL k '
        'JOIN StringIds s ON s.id=k.demangledName WHERE k.start>=? AND k.end<=? '
        'GROUP BY s.value ORDER BY SUM(k.end-k.start) DESC,s.value', (lo, hi))]
    if len(kernels) > 4096 or sum(r['calls'] for r in kernels) != inside:
        raise ValueError('complete bounded kernel-name aggregation required')
    metrics = db.execute('SELECT COUNT(*) FROM GPU_METRICS WHERE timestamp>=? AND timestamp<=?', (lo, hi)).fetchone()[0]
    if not metrics:
        raise ValueError('GPU metrics inside the decode window required')
    return dict(window_start_ns=lo, window_end_ns=hi, window_seconds=(hi-lo)/1e9,
                profiler_API_return_values=[start[4], stop[4]], same_marker_thread=True,
                capture_total_kernels=total, window_kernels=inside,
                outside_window_kernels=total-inside, boundary_straddling_kernels=overlap-inside,
                GPU_metric_rows_in_window=metrics, kernels=kernels,
                summed_kernel_time_is_wall_time=False)
