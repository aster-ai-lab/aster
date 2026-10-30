import concurrent.futures
import itertools
import os
import random
import re
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from aster_literal_scan import LiteralScan, install, exact_mark_pattern

def original_mark(text, tags):
    for tag, marker in tags.items():
        text = text.replace(tag, marker)
    return text

def original_has(value, tags):
    if isinstance(value, str):
        return any(tag in value for tag in tags)
    if isinstance(value, dict):
        return any(original_has(item, tags) for item in value.values())
    if isinstance(value, list):
        return any(original_has(item, tags) for item in value)
    return False

def tags_for(values):
    return {tag: chr(0xf1000 + i) for i, tag in enumerate(sorted(values, key=len, reverse=True))}

def original_unmark(prompt, tags):
    marks = {marker: tag for tag, marker in tags.items()}
    pattern = re.compile('|'.join(map(re.escape, marks)))
    out, spans, position, length = [], [], 0, 0
    for match in pattern.finditer(prompt):
        out.append(prompt[position:match.start()])
        length += match.start() - position
        tag = marks[match.group(0)]
        out.append(tag)
        spans.append((length, length + len(tag)))
        length += len(tag)
        position = match.end()
    out.append(prompt[position:])
    return ''.join(out), spans

class LiteralScanTests(unittest.TestCase):
    def make(self, **kwargs):
        return LiteralScan(original_has, original_mark, original_unmark, **kwargs)

    def test_unmark_exact_text_and_unicode_spans(self):
        tags = tags_for(['<think>', '</think>', '你好', '<｜User｜>'])
        cache = self.make()
        values = list(tags.values())
        for prompt in ['', 'plain', ''.join(values), '🙂' + values[0] + '你好' + values[1], values[2] * 20]:
            self.assertEqual(cache.unmark(prompt, tags), original_unmark(prompt, tags))

    def test_unmark_declared_ranges_never_match_gaps(self):
        values = [0x2d, 0x5d, 0x5e, 0xf0001, 0xf0002, 0xf0004, 0x10ffff]
        pattern = exact_mark_pattern(map(chr, values))
        for value in [0, 44, 45, 46, 93, 94, 95, 0xf0000, 0xf0001, 0xf0002, 0xf0003, 0xf0004, 0xf0005, 0x10ffff]:
            self.assertEqual(pattern.fullmatch(chr(value)) is not None, value in values)

    def test_unmark_falls_back_with_unsafe_maps(self):
        tags = {'bca': chr(0xf0001), 'ab': chr(0xf0002)}
        cache = self.make()
        self.assertFalse(cache.prime(tags))
        self.assertEqual(cache.unmark(chr(0xf0001) + 'x', tags), original_unmark(chr(0xf0001) + 'x', tags))

    def test_unmark_many_markers_and_non_mark_unicode(self):
        tags = tags_for([f'<declared_{i:04d}>' for i in range(1600)])
        cache = self.make()
        self.assertTrue(cache.prime(tags))
        prompt = ('🙂ordinary code;\n' * 1000) + ''.join(list(tags.values())[::37]) + chr(0xf0fff)
        self.assertEqual(cache.unmark(prompt, tags), original_unmark(prompt, tags))

    def check(self, cache, tags, text):
        self.assertEqual(cache.mark(text, tags), original_mark(text, tags))
        self.assertEqual(cache.has(text, tags), original_has(text, tags))

    def test_declared_literals_unicode_and_repetitions(self):
        tags = tags_for(['<think>', '</think>', '<｜User｜>', '<unused_0>', '你好', '(a.+)'])
        cache = self.make()
        self.assertTrue(cache.prime(tags))
        for text in ['', 'plain', '你好 <think>x</think>', '<｜User｜>' * 8, '(a.+) aaaa', '🙂' + chr(0xf1000)]:
            self.check(cache, tags, text)

    def test_crossing_priority_uses_original_semantics(self):
        tags = {'bca': chr(0xf1000), 'ab': chr(0xf1001)}
        cache = self.make()
        self.assertFalse(cache.prime(tags))
        self.check(cache, tags, 'abca ab bca')

    def test_prefix_priority_and_self_overlap(self):
        for values in [['abc', 'ab'], ['aaa', 'aa'], ['aba', 'ab']]:
            tags = tags_for(values)
            cache = self.make()
            for size in range(8):
                for letters in itertools.product('ab', repeat=size):
                    self.check(cache, tags, ''.join(letters))

    def test_unsorted_priority_uses_original(self):
        tags = {'a': chr(0xf1000), 'ba': chr(0xf1001)}
        cache = self.make()
        self.assertFalse(cache.prime(tags))
        self.check(cache, tags, 'ba')

    def test_replacements_cannot_create_a_later_match(self):
        marker = chr(0xf1000)
        tags = {'abc': marker, marker: chr(0xf1001)}
        cache = self.make()
        self.assertFalse(cache.prime(tags))
        self.check(cache, tags, 'abc')

    def test_model_map_changes_are_keyed_by_complete_values(self):
        tags = tags_for(['<one>'])
        cache = self.make()
        first = cache.mark('<one>', tags)
        tags['<one>'] = chr(0xf1010)
        self.assertNotEqual(first, cache.mark('<one>', tags))
        self.check(cache, tags, '<one>')

    def test_cache_and_node_bounds(self):
        cache = self.make(max_entries=2, max_bytes=4096)
        for number in range(12):
            self.check(cache, tags_for([f'<{number}>']), 'text')
            self.assertLessEqual(cache.stats()['entries'], 2)
            self.assertLessEqual(cache.stats()['accounted_bytes'], 4096)
        tiny = self.make(max_nodes=2)
        self.assertFalse(tiny.prime(tags_for(['<long>'])))
        self.check(tiny, tags_for(['<long>']), '<long>')

    def test_nonstandard_objects_keep_their_original_behavior(self):
        class Text(str):
            def replace(self, old, new):
                return 'custom'
        class Mapping(dict):
            pass
        cache = self.make()
        tags = tags_for(['x'])
        self.check(cache, tags, Text('x'))
        self.check(cache, Mapping(tags), 'x')
        for value in [None, 3, {'v': ['x']}, ['plain', {'v': 'x'}]]:
            self.assertEqual(cache.has(value, tags), original_has(value, tags))

    def test_concurrent_calls_keep_exact_outputs(self):
        cache = self.make()
        tags = tags_for(['<think>', '</think>'])
        text = 'a<think>b</think>c' * 32
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
            outputs = list(executor.map(lambda _: cache.mark(text, tags), range(64)))
        self.assertTrue(all(value == original_mark(text, tags) for value in outputs))
        self.assertFalse(cache.stats()['request_text_retained'])

    def test_differential_generated_closed_sets(self):
        randomizer = random.Random(731927)
        cache = self.make()
        for _ in range(60):
            words = {''.join(randomizer.choices('abc', k=randomizer.randrange(1, 6))) for _ in range(6)}
            tags = tags_for(words)
            for _ in range(8):
                self.check(cache, tags, ''.join(randomizer.choices('abc ', k=80)))

    def test_invalid_bounds_and_closed_startup_flag(self):
        for name in ['max_entries', 'max_bytes', 'max_tags', 'max_chars', 'max_nodes']:
            with self.assertRaises(ValueError):
                self.make(**{name: 0})
        for value in ['', '2', 'true', '01', ' 1']:
            with patch.dict(os.environ, ASTER_XQ_LITERAL_SCAN=value):
                with self.assertRaisesRegex(ValueError, 'must be 0 or 1'):
                    install(SimpleNamespace())
        with patch.dict(os.environ, ASTER_XQ_LITERAL_SCAN='0'):
            self.assertIsNone(install(SimpleNamespace()))

if __name__ == '__main__':
    unittest.main()
