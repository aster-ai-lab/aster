import sqlite3
import unittest
from aster_nsys_decode_window_v321 import decode_window


def fixture():
    db = sqlite3.connect(':memory:')
    db.executescript('''
        CREATE TABLE StringIds(id INTEGER,value TEXT);
        CREATE TABLE CUPTI_ACTIVITY_KIND_RUNTIME(start INTEGER,end INTEGER,globalTid INTEGER,nameId INTEGER,returnValue INTEGER);
        CREATE TABLE CUPTI_ACTIVITY_KIND_KERNEL(start INTEGER,end INTEGER,demangledName INTEGER);
        CREATE TABLE GPU_METRICS(timestamp INTEGER,typeId INTEGER,metricId INTEGER,value REAL);
        INSERT INTO StringIds VALUES(1,'cudaProfilerStart_v4000'),(2,'cudaProfilerStop_v4000'),(3,'observed kernel');
        INSERT INTO CUPTI_ACTIVITY_KIND_RUNTIME VALUES(100,110,4,1,0),(200,210,4,2,0);
        INSERT INTO CUPTI_ACTIVITY_KIND_KERNEL VALUES(120,180,3);
        INSERT INTO GPU_METRICS VALUES(150,1,1,30.0),(220,1,1,40.0);
    ''')
    return db


class WindowTests(unittest.TestCase):
    def test_exact_window_excludes_other_metrics_and_preserves_literal_names(self):
        with fixture() as db:
            r = decode_window(db, require_all_kernels=True)
            self.assertEqual((r['window_start_ns'], r['window_end_ns'], r['window_kernels'], r['GPU_metric_rows_in_window']), (110,200,1,1))
            self.assertEqual(r['kernels'], [dict(name='observed kernel',calls=1,summed_ns=60)])

    def test_no_guessing_versioned_api_names(self):
        with fixture() as db:
            db.execute("UPDATE StringIds SET value='cudaProfilerStart_unknown' WHERE id=1")
            with self.assertRaises(ValueError): decode_window(db)

    def test_duplicate_markers_refused(self):
        with fixture() as db:
            db.execute('INSERT INTO CUPTI_ACTIVITY_KIND_RUNTIME VALUES(90,95,4,1,0)')
            with self.assertRaises(ValueError): decode_window(db)

    def test_failed_API_refused(self):
        with fixture() as db:
            db.execute('UPDATE CUPTI_ACTIVITY_KIND_RUNTIME SET returnValue=1 WHERE nameId=2')
            with self.assertRaises(ValueError): decode_window(db)

    def test_other_thread_refused(self):
        with fixture() as db:
            db.execute('UPDATE CUPTI_ACTIVITY_KIND_RUNTIME SET globalTid=5 WHERE nameId=2')
            with self.assertRaises(ValueError): decode_window(db)

    def test_straddling_kernel_refused(self):
        with fixture() as db:
            db.execute('INSERT INTO CUPTI_ACTIVITY_KIND_KERNEL VALUES(190,220,3)')
            with self.assertRaises(ValueError): decode_window(db)

    def test_prefill_can_be_excluded_but_smoke_requires_all(self):
        with fixture() as db:
            db.execute('INSERT INTO CUPTI_ACTIVITY_KIND_KERNEL VALUES(20,40,3)')
            self.assertEqual(decode_window(db)['outside_window_kernels'],1)
            with self.assertRaises(ValueError): decode_window(db,require_all_kernels=True)

    def test_no_metrics_or_missing_schema_refused(self):
        with fixture() as db:
            db.execute('DELETE FROM GPU_METRICS')
            with self.assertRaises(ValueError): decode_window(db)
        with fixture() as db:
            db.execute('DROP TABLE GPU_METRICS')
            with self.assertRaises(ValueError): decode_window(db)


if __name__ == '__main__': unittest.main()
