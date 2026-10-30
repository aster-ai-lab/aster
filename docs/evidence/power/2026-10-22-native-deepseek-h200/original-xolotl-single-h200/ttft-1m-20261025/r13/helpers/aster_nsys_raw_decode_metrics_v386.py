"""Summarize exact observed metric IDs inside an already validated decode window."""
import math

def decode_raw_metrics(db, window):
    lo=window['window_start_ns'];hi=window['window_end_ns']
    if type(lo) is not int or type(hi) is not int or not 1e9 <= hi-lo <= 180e9:
        raise ValueError('decode metric window must be one to 180 seconds')
    required={'GPU_METRICS':{'timestamp','typeId','metricId','value'},
              'TARGET_INFO_GPU_METRICS':{'typeId','sourceId','typeName','metricId','metricName'}}
    for table, columns in required.items():
        actual={r[1] for r in db.execute('PRAGMA table_info('+table+')')}
        if not columns <= actual:raise ValueError('observed decode metric schema differs')
    n=db.execute('SELECT COUNT(*) FROM GPU_METRICS WHERE timestamp>=? AND timestamp<=?',(lo,hi)).fetchone()[0]
    if not 0<n<=1000000 or n!=window['GPU_metric_rows_in_window']:raise ValueError('bounded exact decode metric count differs')
    types=db.execute('SELECT COUNT(DISTINCT typeId) FROM GPU_METRICS WHERE timestamp>=? AND timestamp<=?',(lo,hi)).fetchone()[0]
    if types!=1:raise ValueError('exactly one observed decode GPU metric type required')
    if db.execute('SELECT 1 FROM TARGET_INFO_GPU_METRICS GROUP BY typeId,metricId HAVING COUNT(*)!=1 LIMIT 1').fetchone():raise ValueError('ambiguous observed metric identity')
    if db.execute('SELECT 1 FROM GPU_METRICS WHERE timestamp>=? AND timestamp<=? AND (typeof(timestamp)!=\'integer\' OR typeof(value) NOT IN (\'integer\',\'real\')) LIMIT 1',(lo,hi)).fetchone():raise ValueError('numeric observed metric storage required')
    if db.execute('SELECT 1 FROM GPU_METRICS g LEFT JOIN TARGET_INFO_GPU_METRICS m ON g.typeId=m.typeId AND g.metricId=m.metricId WHERE g.timestamp>=? AND g.timestamp<=? AND m.metricName IS NULL LIMIT 1',(lo,hi)).fetchone():raise ValueError('observed metric identity missing')
    rows=db.execute('''SELECT g.typeId,g.metricId,m.sourceId,m.typeName,m.metricName,
        COUNT(*),MIN(g.timestamp),MAX(g.timestamp),MIN(g.value),AVG(g.value),MAX(g.value)
        FROM GPU_METRICS g JOIN TARGET_INFO_GPU_METRICS m ON g.typeId=m.typeId AND g.metricId=m.metricId
        WHERE g.timestamp>=? AND g.timestamp<=?
        GROUP BY g.typeId,g.metricId,m.sourceId,m.typeName,m.metricName ORDER BY g.typeId,g.metricId''',(lo,hi)).fetchall()
    if not 0<len(rows)<=128:raise ValueError('bounded observed metric identities required')
    out=[]
    for tid,mid,sid,typ,name,count,first,last,minimum,mean,maximum in rows:
        if type(name) is not str or not 0<len(name)<=256 or type(typ) is not str or len(typ)>256:raise ValueError('bounded literal metric names required')
        if any(type(v) not in (int,float) or not math.isfinite(v) for v in [minimum,mean,maximum]):raise ValueError('finite observed metrics required')
        if count<5 or lo>first or hi<last or first-lo>250000000 or hi-last>250000000:raise ValueError('metric samples must cover the full decode window within 250ms')
        out.append(dict(type_id=tid,metric_id=mid,source_id=sid,type_name=typ,metric_name=name,samples=count,first_ns=first,last_ns=last,minimum=minimum,mean=mean,maximum=maximum))
    if sum(r['samples'] for r in out)!=n:raise ValueError('complete decode metric cardinality differs')
    return dict(metric_rows=n,window_seconds=(hi-lo)/1e9,metrics=out,literal_names_and_units_preserved=True,per_kernel_attribution_claimed=False,clock_unit_conversion_applied=False,physical_counter_values_qualified=False,raw_signed_values_preserved=True,negative_values_present=any(r['minimum']<0 for r in out))
