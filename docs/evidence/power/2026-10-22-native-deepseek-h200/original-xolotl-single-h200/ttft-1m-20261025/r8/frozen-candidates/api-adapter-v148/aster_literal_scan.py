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
SERVER_SHA256 = '04abb76be1608b0c718891fb1486f8a05faada618948f963b222b8f60d62417b'

def exact_mark_pattern(markers):
    """Compact only contiguous declared code points; no gaps or inferred markers."""
    values = sorted(ord(char) for char in markers)
    parts = []
    index = 0
    while index < len(values):
        start = end = values[index]
        index += 1
        while index < len(values) and values[index] == end + 1:
            end = values[index]
            index += 1
        parts.append('\\U%08x' % start + ('-\\U%08x' % end if start != end else ''))
    return re.compile('[' + ''.join(parts) + ']')

class LiteralScan:
    def __init__(self, original_has, original_mark, original_unmark=None, *, max_entries=2, max_bytes=2*1024**2,
                 max_tags=4096, max_chars=65536, max_nodes=32768):
        if min(max_entries, max_bytes, max_tags, max_chars, max_nodes) <= 0:
            raise ValueError('literal scan bounds must be positive')
        self._has = original_has
        self._mark = original_mark
        self._unmark = original_unmark
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
                    unmarks = {mark: tag for tag, mark in signature}
                    result = (pattern, dict(signature), exact_mark_pattern(unmarks), unmarks)
                except re.error:
                    result = None
            charge = sys.getsizeof(signature) + sum(sys.getsizeof(pair) + sys.getsizeof(pair[0]) + sys.getsizeof(pair[1]) for pair in signature) + 256
            if result is not None:
                charge += sum(sys.getsizeof(item) for item in result)
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
                pattern, replacements = entry[:2]
                return pattern.sub(lambda match: replacements[match.group(0)], text)
        return self._mark(text, tags)

    def unmark(self, prompt, tags):
        if type(prompt) is str:
            entry = self._entry(tags)
            if entry is not None:
                pattern, marks = entry[2:]
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
        if self._unmark is None:
            raise RuntimeError('exact original unmark fallback was not supplied')
        return self._unmark(prompt, tags)

    def stats(self):
        with self._lock:
            return dict(entries=len(self._cache), accounted_bytes=self._bytes,
                        compiled_maps=self._compiled, fallback_maps=self._fallbacks,
                        request_text_retained=False)

def install(frontend, server=None):
    mode = os.environ.get('ASTER_XQ_LITERAL_SCAN', '0')
    if mode not in ('0', '1'):
        raise ValueError('ASTER_XQ_LITERAL_SCAN must be 0 or 1')
    if mode == '0':
        return None
    path = Path(frontend.__file__)
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != FRONTEND_SHA256:
        logging.error('literal scan disabled: pinned frontend source SHA differs')
        return None
    if server is not None:
        path = Path(server.__file__)
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != SERVER_SHA256:
            logging.error('literal scan disabled: pinned server source SHA differs')
            return None
    installed = getattr(frontend, '_aster_literal_scan', None)
    if installed is not None:
        return installed
    if server is not None and server.unmark_think_literals is not frontend.unmark_think_literals:
        logging.error('literal scan disabled: pinned server unmark alias differs')
        return None
    cache = LiteralScan(frontend._has_tag, frontend._mark, frontend.unmark_think_literals)
    def has(value, tags=frontend.LITERAL_TAGS):
        return cache.has(value, tags)
    def mark(text, tags=frontend.LITERAL_TAGS):
        return cache.mark(text, tags)
    def unmark(prompt, tags=frontend.LITERAL_TAGS):
        return cache.unmark(prompt, tags)
    frontend._has_tag = has
    frontend._mark = mark
    frontend.unmark_think_literals = unmark
    if server is not None:
        server.unmark_think_literals = unmark
    frontend._aster_literal_scan = cache
    return cache
