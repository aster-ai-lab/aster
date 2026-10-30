import concurrent.futures
import time
import unittest
from aster_token_cache import ExactEncodeCache


class ExactTokenCacheTests(unittest.TestCase):
    def make(self, **kwargs):
        calls = []

        def original(text, parse_special=False, plain=()):
            calls.append((text, parse_special, plain))
            return [1000 + int(bool(parse_special)), len(plain)] + [ord(char) for char in text]

        return ExactEncodeCache(original, min_chars=0, **kwargs), calls

    def test_exact_text_and_special_mode_have_distinct_results(self):
        cache, calls = self.make()
        for text in ['你好', '你好吗', '你好']:
            self.assertEqual(cache.encode(text, True), [1001, 0] + list(map(ord, text)))
        self.assertEqual(cache.encode('你好', False), [1000, 0] + list(map(ord, '你好')))
        self.assertEqual(len(calls), 3)
        self.assertEqual(cache.stats()['hits'], 1)

    def test_returned_lists_cannot_mutate_owned_cache(self):
        cache, _ = self.make()
        first = cache.encode('hello', True)
        expected = first.copy()
        first[0] = -1
        second = cache.encode('hello', True)
        self.assertEqual(second, expected)
        second.clear()
        self.assertEqual(cache.encode('hello', True), expected)

    def test_literal_span_override_and_non_boolean_flags_bypass(self):
        cache, calls = self.make()
        for _ in range(2):
            self.assertEqual(cache.encode('<think>', True, ((0, 7),)), [1001, 1] + list(map(ord, '<think>')))
            cache.encode('hello', 1)
            cache.encode('hello', True, [])
        self.assertEqual(len(calls), 6)
        self.assertEqual(cache.stats()['entries'], 0)

    def test_instances_do_not_share_models_or_text(self):
        one = ExactEncodeCache(lambda text, **kwargs: [11], min_chars=0)
        two = ExactEncodeCache(lambda text, **kwargs: [22], min_chars=0)
        for _ in range(2):
            self.assertEqual(one.encode('same'), [11])
            self.assertEqual(two.encode('same'), [22])
        self.assertEqual(one.stats()['entries'], 1)
        self.assertEqual(two.stats()['entries'], 1)

    def test_lru_entry_bound_keeps_recent_request(self):
        cache, calls = self.make(max_entries=2)
        for text in ['a', 'b', 'a', 'c', 'a']:
            cache.encode(text)
        self.assertEqual(len(calls), 3)
        cache.encode('b')
        self.assertEqual(len(calls), 4)
        self.assertEqual(cache.stats()['entries'], 2)

    def test_byte_bound_and_oversize_input_bypass(self):
        cache, _ = self.make(max_bytes=1024, max_input_bytes=64)
        for number in range(100):
            cache.encode(str(number) * 8)
            self.assertLessEqual(cache.stats()['accounted_bytes'], 1024)
        before = cache.stats()['entries']
        self.assertEqual(cache.encode('x' * 65), [1000, 0] + [120] * 65)
        self.assertEqual(cache.stats()['entries'], before)

    def test_original_errors_are_not_cached(self):
        calls = []

        def original(text, **kwargs):
            calls.append(text)
            if len(calls) == 1:
                raise ValueError('original encoder error')
            return [23]

        cache = ExactEncodeCache(original, min_chars=0)
        with self.assertRaisesRegex(ValueError, 'original encoder error'):
            cache.encode('text')
        self.assertEqual(cache.encode('text'), [23])
        self.assertEqual(cache.encode('text'), [23])
        self.assertEqual(len(calls), 2)

    def test_unexpected_original_ids_keep_their_value_and_type(self):
        for ids in [[-1], [True], [2**32], (1, 2)]:
            cache = ExactEncodeCache(lambda text, **kwargs: ids, min_chars=0)
            for _ in range(2):
                self.assertIs(cache.encode('text'), ids)
            self.assertEqual(cache.stats()['entries'], 0)

    def test_concurrent_requests_keep_private_return_lists(self):
        def original(text, **kwargs):
            time.sleep(.001)
            return [1000, 3, 5]

        cache = ExactEncodeCache(original, min_chars=0)
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            rows = list(pool.map(lambda _: cache.encode('same'), range(32)))
        self.assertTrue(all(row == [1000, 3, 5] for row in rows))
        self.assertEqual(len({id(row) for row in rows}), 32)
        self.assertEqual(cache.stats()['entries'], 1)

    def test_invalid_memory_configuration_refuses(self):
        for kwargs in [dict(max_bytes=0), dict(max_entries=0), dict(min_chars=-1), dict(max_input_bytes=0)]:
            with self.assertRaises(ValueError):
                ExactEncodeCache(lambda text, **opts: [], **kwargs)


if __name__ == '__main__':
    unittest.main()
