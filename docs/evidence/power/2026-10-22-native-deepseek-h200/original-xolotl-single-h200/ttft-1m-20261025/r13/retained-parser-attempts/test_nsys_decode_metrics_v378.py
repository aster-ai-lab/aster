import sqlite3,unittest
from aster_nsys_decode_metrics_v378 import decode_metrics

def fixture():
    db=sqlite3.connect(':memory:')
    db.executescript('CREATE TABLE GPU_METRICS(timestamp INTEGER,typeId INTEGER,metricId INTEGER,value REAL); CREATE TABLE TARGET_INFO_GPU_METRICS(typeId INTEGER,sourceId INTEGER,typeName TEXT,metricId INTEGER,metricName TEXT);')
    db.execute('INSERT INTO TARGET_INFO_GPU_METRICS VALUES(1,9,\'observed GPU\',4,\'observed Clock [MHz]\')')
    db.executemany('INSERT INTO GPU_METRICS VALUES(?,1,4,?)',[(t,1900000000.0) for t in range(0,80000000001,100000000)])
    w=dict(window_start_ns=10000000000,window_end_ns=70000000000,GPU_metric_rows_in_window=601)
    return db,w

class MetricsTests(unittest.TestCase):
    def test_actual_long_window_and_literal_names(self):
        db,w=fixture()
        with db:
            r=decode_metrics(db,w);self.assertEqual(r['window_seconds'],60);self.assertEqual(r['metric_rows'],601)
            self.assertEqual(r['metrics'][0]['metric_name'],'observed Clock [MHz]');self.assertEqual(r['metrics'][0]['mean'],1900000000.0)
            self.assertFalse(r['clock_unit_conversion_applied'])
    def test_outside_window_not_averaged(self):
        db,w=fixture();db.execute('UPDATE GPU_METRICS SET value=1 WHERE timestamp<10000000000 OR timestamp>70000000000')
        self.assertEqual(decode_metrics(db,w)['metrics'][0]['minimum'],1900000000.0)
    def test_duplicate_metadata_refused(self):
        db,w=fixture();db.execute('INSERT INTO TARGET_INFO_GPU_METRICS SELECT * FROM TARGET_INFO_GPU_METRICS')
        with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_missing_identity_refused(self):
        db,w=fixture();db.execute('DELETE FROM TARGET_INFO_GPU_METRICS')
        with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_two_devices_refused(self):
        db,w=fixture();db.execute('UPDATE GPU_METRICS SET typeId=2 WHERE timestamp=10000000000')
        with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_negative_or_nonfinite_refused(self):
        for bad in [-1,float('inf'),'bad']:
            db,w=fixture();db.execute('UPDATE GPU_METRICS SET value=? WHERE timestamp=10000000000',(bad,))
            with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_count_mismatch_refused(self):
        db,w=fixture();w['GPU_metric_rows_in_window']=600
        with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_partial_coverage_refused(self):
        db,w=fixture();db.execute('DELETE FROM GPU_METRICS WHERE timestamp<11000000000');w['GPU_metric_rows_in_window']=591
        with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_wrong_or_oversized_window_refused(self):
        for lo,hi in [(0,200000000000),(0,500000000),(True,70000000000)]:
            db,w=fixture();w.update(window_start_ns=lo,window_end_ns=hi)
            with self.assertRaises(ValueError):decode_metrics(db,w)
    def test_missing_schema_refused(self):
        db,w=fixture();db.execute('DROP TABLE GPU_METRICS')
        with self.assertRaises(ValueError):decode_metrics(db,w)

if __name__=='__main__':unittest.main()
