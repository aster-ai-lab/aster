"""Per-tokenizer bounded memo of exact text and special-token mode.

No prefix inference, normalization or persistent storage. Literal-span overrides
use the original encoder. Every caller receives a fresh list, including hits.
"""
from array import array
from collections import OrderedDict
import sys
import threading


class ExactEncodeCache:
    def __init__(self, encode, *, max_bytes=32 * 1024**2, max_entries=4,
                 min_chars=4096, max_input_bytes=8 * 1024**2):
        if max_bytes <= 0 or max_entries <= 0 or min_chars < 0 or max_input_bytes <= 0:
            raise ValueError('exact token cache requires positive byte/entry/input bounds and nonnegative minimum chars')
        self._encode = encode
        self._max_bytes = max_bytes
        self._max_entries = max_entries
        self._min_chars = min_chars
        self._max_input_bytes = max_input_bytes
        self._entries = OrderedDict()
        self._lock = threading.Lock()
        self._bytes = self._hits = self._misses = self._admissions = 0

    def encode(self, text, parse_special=False, plain=()):
        # Preserve the upstream encoder's complete interface. Cache only the
        # ordinary declared path; no interpretation of arbitrary literal spans,
        # truthy flags, str subclasses or alternative input objects.
        if (type(text) is not str or type(parse_special) is not bool or
                type(plain) is not tuple or plain or len(text) < self._min_chars):
            return self._encode(text, parse_special=parse_special, plain=plain)
        try:
            raw = text.encode('utf-8')
        except UnicodeEncodeError:
            return self._encode(text, parse_special=parse_special, plain=plain)
        if len(raw) > self._max_input_bytes:
            return self._encode(text, parse_special=parse_special, plain=plain)
        # UTF-8 is injective for accepted Unicode text. Store and compare the
        # entire byte string, never a hash-only identity or guessed prefix.
        key = (parse_special, raw)
        with self._lock:
            entry = self._entries.get(key)
            if entry is not None:
                self._entries.move_to_end(key)
                self._hits += 1
                packed = entry[0]
            else:
                self._misses += 1
                packed = None
        if packed is not None:
            return list(packed)
        ids = self._encode(text, parse_special=parse_special, plain=plain)
        # Optimization cannot change the original result's value or type.
        # Unexpected IDs remain an uncached result of the original encoder.
        if type(ids) is not list or any(type(value) is not int or value < 0 or value > 0xffffffff for value in ids):
            return ids
        packed = array('I', ids)
        charge = sys.getsizeof(key) + sys.getsizeof(raw) + sys.getsizeof(packed) + 256
        if charge > self._max_bytes:
            return ids
        with self._lock:
            # Another request may have completed the same encode meanwhile.
            # Its immutable owned packed data already supplies this key.
            if key not in self._entries:
                while self._entries and (len(self._entries) >= self._max_entries or self._bytes + charge > self._max_bytes):
                    _, (_, old_charge) = self._entries.popitem(last=False)
                    self._bytes -= old_charge
                self._entries[key] = (packed, charge)
                self._bytes += charge
                self._admissions += 1
        return ids

    def stats(self):
        with self._lock:
            return dict(entries=len(self._entries), accounted_bytes=self._bytes,
                        hits=self._hits, misses=self._misses, admissions=self._admissions)
