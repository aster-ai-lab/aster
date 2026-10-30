import csv
import io
import unittest
from aster_ncu_metrics_v171 import METRICS
from aster_ncu_query_v178 import HEADER,confirmed_metrics

def fixture(rows=None,header=HEADER):
    out=io.StringIO();out.write('==WARNING== public installed prelude\n')
    writer=csv.writer(out);writer.writerow(header)
    writer.writerows(rows if rows is not None else [[metric,'Counter','unit','description','1'] for metric in METRICS])
    return out.getvalue()

class Query(unittest.TestCase):
    def test_exact_actual_columns(self):
        self.assertEqual(confirmed_metrics(fixture()),list(METRICS))
    def test_missing_and_unavailable(self):
        rows=[[metric,'Counter','unit','description','1'] for metric in METRICS]
        for modified in [rows[:-1],rows[:-1]+[[METRICS[-1],'Counter','unit','description','0']]]:
            with self.assertRaises(ValueError):confirmed_metrics(fixture(modified))
    def test_other_model_and_column_contract(self):
        for header in [HEADER[:-1]+('NVIDIA H100',),HEADER[:-1]+('NVIDIA H200 ','NVIDIA H200')]:
            with self.assertRaises(ValueError):confirmed_metrics(fixture(header=header))
    def test_duplicate_and_malformed(self):
        rows=[[metric,'Counter','unit','description','1'] for metric in METRICS]
        for modified in [rows+[rows[0]],rows+[['wrong-width']]]:
            with self.assertRaises(ValueError):confirmed_metrics(fixture(modified))
    def test_bounds(self):
        for text in ['x'*262145,fixture([['unrequested','Counter','unit','x'*8193,'1']]),fixture([['unrequested','Counter','unit','description','1']]*1025)]:
            with self.assertRaises(ValueError):confirmed_metrics(text)
    def test_unrequested_metrics_do_not_substitute(self):
        rows=[[metric,'Counter','unit','description','1'] for metric in METRICS[:-1]]
        rows.append(['smsp__warps_eligible.sum','Counter','unit','description','1'])
        with self.assertRaises(ValueError):confirmed_metrics(fixture(rows))

if __name__=='__main__':unittest.main()
