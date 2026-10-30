import csv
import io
import unittest
from aster_ncu_metrics_v171 import METRICS, numeric_rows, family_counts

def fixture(values=None):
    stream=io.StringIO();writer=csv.writer(stream)
    writer.writerow(['ID','Kernel Name','Metric Name','Metric Unit','Metric Value'])
    for index,metric in enumerate(METRICS):
        writer.writerow(['0','void flash_attn_ext_f16<512>(float*)',metric,'unit',(values or {}).get(index,'1.25')])
    return stream.getvalue()

class NumericMetricsTests(unittest.TestCase):
    def test_exact_csv_and_declared_symbol(self):
        rows=numeric_rows(fixture());self.assertEqual(len(rows),8)
        self.assertEqual(family_counts(rows)['flash_attn_ext_f16'],1)
        self.assertTrue(all(row['value']==1.25 for row in rows))
    def test_missing_header_or_counter_refused(self):
        for text in ['','no profiler data','\n'.join(fixture().splitlines()[:-1])]:
            with self.assertRaises(ValueError):numeric_rows(text)
    def test_nonfinite_negative_and_invalid_numbers_refused(self):
        for value in ['nan','inf','-1','unknown','N/A']:
            with self.subTest(value=value),self.assertRaises(ValueError):numeric_rows(fixture({0:value}))
    def test_duplicate_or_malformed_row_refused(self):
        text=fixture();last=text.splitlines()[-1]+'\n'
        with self.assertRaises(ValueError):numeric_rows(text+last)
        with self.assertRaises(ValueError):numeric_rows(text+'broken,row\n')
    def test_bounded_text_and_symbols(self):
        with self.assertRaises(ValueError):numeric_rows('x'*(2*1024**2+1))
        with self.assertRaises(ValueError):numeric_rows(fixture().replace('void flash_attn_ext_f16<512>(float*)','x'*8193))
    def test_numeric_scaling_prelude_and_literal_names(self):
        rows=numeric_rows('==PROF== importing owned report\n'+fixture({0:'1,234.5',1:'2.5e1'}))
        self.assertEqual(rows[0]['value'],1234.5);self.assertEqual(rows[1]['value'],25)
        for row in rows:row['symbol']='unknown_attention(float*)'
        self.assertTrue(all(value==0 for value in family_counts(rows).values()))

if __name__=='__main__':unittest.main()
