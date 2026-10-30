"""Read the installed query's exact H200 support column for eight named metrics."""
import csv
import io
from aster_ncu_metrics_v171 import METRICS

HEADER=('Metric name','Metric type','Metric unit','Metric description','NVIDIA H200')

def confirmed_metrics(text):
    if type(text) is not str or len(text.encode())>262144:
        raise ValueError('bounded H200 metric suffix CSV exceeded')
    header=False;confirmed=set();count=0
    for fields in csv.reader(io.StringIO(text)):
        if not header:
            if tuple(fields)==HEADER:header=True
            continue
        if not fields or not any(fields):continue
        count+=1
        if count>1024 or len(fields)!=len(HEADER):raise ValueError('bounded exact H200 metric suffix CSV differs')
        if any(len(value)>8192 for value in fields):raise ValueError('bounded H200 metric suffix field exceeded')
        if fields[0] not in METRICS:continue
        if fields[0] in confirmed:raise ValueError('duplicate exact H200 supported metric')
        if fields[-1]!='1':raise ValueError('requested metric is not supported by the actual H200')
        confirmed.add(fields[0])
    if not header or confirmed!=set(METRICS):raise ValueError('exact H200 requested metric support is incomplete')
    return list(METRICS)
