import csv
import io
import unittest
from aster_ncu_metrics_v171 import METRICS
from aster_ncu_wide_metrics_v206 import numeric_wide_rows


def fixture():
    return [['ID', 'Kernel Name', *METRICS], ['', '', 'ns', '%', '%', '%', 'byte', 'byte', 'warp', '%'],
        ['0', 'aster_h200_counter_smoke(unsigned int *, int)', '2016', '4.70', '0', '21.08', '0', '384', '0.25', '17.36']]


def encode(rows):
    out = io.StringIO(); csv.writer(out).writerows(rows); return out.getvalue()


class WideCSV(unittest.TestCase):
    def test_observed_schema_and_units(self):
        rows = numeric_wide_rows('==WARNING== retained preamble\n' + encode(fixture()))
        self.assertEqual(len(rows), 8)
        self.assertEqual(rows[0]['value'], 2016)
        self.assertEqual(rows[0]['unit'], 'ns')
        self.assertEqual({r['metric'] for r in rows}, set(METRICS))

    def test_missing_metric_is_refused(self):
        rows = [row[:-1] for row in fixture()]
        with self.assertRaises(ValueError): numeric_wide_rows(encode(rows))

    def test_duplicate_column_is_refused(self):
        rows = fixture(); rows[0].append(rows[0][-1])
        with self.assertRaises(ValueError): numeric_wide_rows(encode(rows))

    def test_units_required_before_data(self):
        rows = fixture(); del rows[1]
        with self.assertRaises(ValueError): numeric_wide_rows(encode(rows))

    def test_duplicate_kernel_id_is_refused(self):
        rows = fixture(); rows.append(list(rows[-1]))
        with self.assertRaises(ValueError): numeric_wide_rows(encode(rows))

    def test_nonfinite_and_negative_are_refused(self):
        for invalid in ['nan', 'inf', '-1']:
            rows = fixture(); rows[-1][-1] = invalid
            with self.subTest(invalid=invalid), self.assertRaises(ValueError): numeric_wide_rows(encode(rows))

    def test_bounded_and_ragged_input(self):
        rows = fixture(); rows[-1].append('extra')
        with self.assertRaises(ValueError): numeric_wide_rows(encode(rows))
        with self.assertRaises(ValueError): numeric_wide_rows('x' * (2 * 1024**2 + 1))


if __name__ == '__main__': unittest.main()
