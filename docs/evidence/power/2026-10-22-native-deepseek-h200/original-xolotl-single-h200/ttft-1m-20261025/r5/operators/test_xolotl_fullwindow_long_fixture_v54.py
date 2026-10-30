"""Hermetic refusal cases for the actual1M-plus1024 client acceptance boundary."""
import copy
import hashlib
import json
import unittest
from xolotl_fullwindow_long_fixture_v54 import (
    CONTEXT, EXPECTED, INPUT, OUTPUT, SLACK, check_completion, check_record_prefix,
)


class FullWindowAcceptanceTests(unittest.TestCase):
    def setUp(self):
        content = json.dumps(EXPECTED) + '\n' + 'Detailed cache consistency explanation. ' * 80
        self.metadata = dict(context_tokens=CONTEXT, actual_prompt_tokens=INPUT,
                             max_output_tokens=OUTPUT, context_safety_tokens=SLACK,
                             total_reserved_tokens=CONTEXT)
        self.row = dict(status=200, done=True, finish_reason='length', content=content,
                        output_sha256=hashlib.sha256(content.encode()).hexdigest(),
                        usage=dict(prompt_tokens=INPUT, completion_tokens=OUTPUT, total_tokens=INPUT + OUTPUT,
                                   prompt_tokens_details=dict(cached_tokens=0)),
                        seconds=1980.0, ttft_seconds=1920.0, first_to_last_text_seconds=60.0)

    def test_complete_cold_and_exact_warm_answer(self):
        digest = check_completion(self.row, self.metadata, cold=True)
        self.row['usage']['prompt_tokens_details']['cached_tokens'] = INPUT
        self.assertEqual(check_completion(self.row, self.metadata, cold=False, reference_sha256=digest), digest)

    def test_wrong_or_partial_record_values_refused(self):
        bad = [
            '{"begin":731927,"middle":408563,"end":1}',
            '{"begin":731927,"middle":408563}',
            '{"begin":731927,"middle":408563,"end":962814,"extra":1}',
            '{"begin":731927,"begin":731927,"middle":408563,"end":962814}',
            '{"begin":true,"middle":408563,"end":962814}',
            '{"begin":731927.0,"middle":408563,"end":962814}',
            '{"begin":NaN,"middle":408563,"end":962814}',
            chr(96) * 3 + 'json\n{"begin":731927,"middle":408563,"end":962814}\n' + chr(96) * 3,
        ]
        for prefix in bad:
            with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                check_record_prefix(prefix + '\n' + 'continuation ' * 200)

    def test_missing_or_inline_continuation_refused(self):
        for suffix in ['', '\nshort', ' ' + 'continuation ' * 200]:
            with self.subTest(suffix=suffix[:30]), self.assertRaises(ValueError):
                check_record_prefix(json.dumps(EXPECTED) + suffix)

    def test_incomplete_http_sse_and_output_budget_refused(self):
        changes = [('status', 400), ('done', False), ('finish_reason', 'stop')]
        for key, value in changes:
            row = dict(self.row, **{key: value})
            with self.subTest(key=key), self.assertRaises(ValueError):
                check_completion(row, self.metadata, cold=True)
        for key, value in [('prompt_tokens', INPUT - 1), ('completion_tokens', 17), ('total_tokens', CONTEXT)]:
            row = copy.deepcopy(self.row)
            row['usage'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                check_completion(row, self.metadata, cold=True)

    def test_reservation_and_cache_refusals(self):
        for key in self.metadata:
            metadata = dict(self.metadata)
            metadata[key] -= 1
            with self.subTest(key=key), self.assertRaises(ValueError):
                check_completion(self.row, metadata, cold=True)
        for cached in [None, True, 1024]:
            row = copy.deepcopy(self.row)
            row['usage']['prompt_tokens_details']['cached_tokens'] = cached
            with self.subTest(cached=cached), self.assertRaises(ValueError):
                check_completion(row, self.metadata, cold=True)
        with self.assertRaises(ValueError):
            check_completion(self.row, self.metadata, cold=False)
        self.row['usage']['prompt_tokens_details']['cached_tokens'] = INPUT + 1
        with self.assertRaises(ValueError):
            check_completion(self.row, self.metadata, cold=False)

    def test_timing_and_identity_refusals(self):
        for key in ['seconds', 'ttft_seconds', 'first_to_last_text_seconds']:
            for value in [None, 0, -1, float('nan'), float('inf'), True]:
                row = dict(self.row, **{key: value})
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    check_completion(row, self.metadata, cold=True)
        with self.assertRaises(ValueError):
            check_completion(self.row, self.metadata, cold=True, reference_sha256='0' * 64)
        with self.assertRaises(ValueError):
            check_completion(dict(self.row, content=self.row['content'] + 'altered'), self.metadata, cold=True)


if __name__ == '__main__':
    unittest.main()
