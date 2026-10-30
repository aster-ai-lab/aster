"""Bounded exact literal matching for the pinned API frontend, opt-in only.

The declared tokenizer literal map is the authority. No names, intent, prefixes
or token boundaries are inferred. Priority-crossing maps use the original code.
No request text is retained in the pattern cache.
"""
from collections import OrderedDict
import hashlib
import logging
import os
from pathlib import Path
import re
import sys
import threading

FRONTEND_SHA256 = '54e7c1e2bc7f2ea52fe05cdd6d5b62d9ef83ef2e2bf8919db8ec417d3cb0713e'

class LiteralScan:
    def __init__(self, original_has, original_mark, *, max_entries=2, max_bytes=2*1024**2,
                 max_tags=4096, max_chars=65536, max_nodes=32768):
        if min(max_entries, max_bytes, max_tags, max_chars, max_nodes) <= 0:
            raise ValueError('literal scan bounds must be positive')
        self._has = original_has
        self._mark = original_mark
        self._limits = (max_entries, max_bytes, max_tags, max_chars, max_nodes)
        self._cache = OrderedDict()
        self._bytes = 0
        self._lock = threading.Lock()
        self._compiled = self._fallbacks = 0

    def _entry(self, tags):
        max_entries, max_bytes, max_tags, max_chars, max_nodes = self._limits
        if type(tags) is not dict or not tags or len(tags) > max_tags:
            return None
        length = 0
        previous = sys.maxsize
        marks = set()
        for tag, mark in tags.items():
            if type(tag) is not str or type(mark) is not str or not tag or len(tag) > 256 or len(mark) != 1:
                return None
            if len(tag) > previous or mark in marks:
                return None
            previous = len(tag)
            length += len(tag)
            if length > max_chars:
                return None
            marks.add(mark)
        if any(any(char in marks for char in tag) for tag in tags):
            return None
        signature = tuple(tags.items())
        with self._lock:
            if signature in self._cache:
                result, charge = self._cache[signature]
                self._cache.move_to_end(signature)
                return result
            # Each node records the earliest priority STRICTLY below the prefix.
            # A later-priority leftmost tag must never overlap an earlier-priority
            # tag beginning inside it. Otherwise a one-pass regex changes str.replace.
            edges = [{}]
            earliest = [sys.maxsize]
            safe = True
            for rank, (tag, _) in enumerate(signature):
                node = 0
                for char in tag:
                    earliest[node] = min(earliest[node], rank)
                    child = edges[node].get(char)
                    if child is None:
                        if len(edges) >= max_nodes:
                            safe = False
                            break
                        child = len(edges)
                        edges[node][char] = child
                        edges.append({})
                        earliest.append(sys.maxsize)
                    node = child
                if not safe:
                    break
            if safe:
                for rank, (tag, _) in enumerate(signature):
                    for offset in range(1, len(tag)):
                        node = 0
                        for char in tag[offset:]:
                            node = edges[node].get(char)
                            if node is None:
                                break
                        if node is not None and earliest[node] < rank:
                            safe = False
                            break
                    if not safe:
                        break
            result = None
            if safe:
                try:
                    pattern = re.compile('|'.join(re.escape(tag) for tag, _ in signature))
                    result = (pattern, dict(signature))
                except re.error:
                    result = None
            charge = sys.getsizeof(signature) + sum(sys.getsizeof(pair) + sys.getsizeof(pair[0]) + sys.getsizeof(pair[1]) for pair in signature) + 256
            if result is not None:
                charge += sys.getsizeof(result[0]) + sys.getsizeof(result[1])
            if charge <= max_bytes:
                while self._cache and (len(self._cache) >= max_entries or self._bytes + charge > max_bytes):
                    _, (_, old) = self._cache.popitem(last=False)
                    self._bytes -= old
                self._cache[signature] = (result, charge)
                self._bytes += charge
            self._compiled += result is not None
            self._fallbacks += result is None
            return result

    def prime(self, tags):
        return self._entry(tags) is not None

    def has(self, value, tags):
        if type(value) is str:
            entry = self._entry(tags)
            if entry is not None:
                return entry[0].search(value) is not None
        return self._has(value, tags)

    def mark(self, text, tags):
        if type(text) is str:
            entry = self._entry(tags)
            if entry is not None:
                pattern, replacements = entry
                return pattern.sub(lambda match: replacements[match.group(0)], text)
        return self._mark(text, tags)

    def stats(self):
        with self._lock:
            return dict(entries=len(self._cache), accounted_bytes=self._bytes,
                        compiled_maps=self._compiled, fallback_maps=self._fallbacks,
                        request_text_retained=False)

def install(frontend):
    mode = os.environ.get('ASTER_XQ_LITERAL_SCAN', '0')
    if mode not in ('0', '1'):
        raise ValueError('ASTER_XQ_LITERAL_SCAN must be 0 or 1')
    if mode == '0':
        return None
    path = Path(frontend.__file__)
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != FRONTEND_SHA256:
        logging.error('literal scan disabled: pinned frontend source SHA differs')
        return None
    installed = getattr(frontend, '_aster_literal_scan', None)
    if installed is not None:
        return installed
    cache = LiteralScan(frontend._has_tag, frontend._mark)
    def has(value, tags=frontend.LITERAL_TAGS):
        return cache.has(value, tags)
    def mark(text, tags=frontend.LITERAL_TAGS):
        return cache.mark(text, tags)
    frontend._has_tag = has
    frontend._mark = mark
    frontend._aster_literal_scan = cache
    return cache
