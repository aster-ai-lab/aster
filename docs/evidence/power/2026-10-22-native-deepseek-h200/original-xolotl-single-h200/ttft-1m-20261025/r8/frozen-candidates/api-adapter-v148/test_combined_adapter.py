"""Actual BPE stream boundaries and strict cache modes, no model or credential."""
import codecs
import json
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'source/tools/v41/serve'))
import serve_ds41 as D


class CombinedAdapterTests(unittest.TestCase):
    def test_closed_memo_mode(self):
        with patch.dict(os.environ, {}, clear=True):
            self.assertFalse(D._aster_exact_memo_enabled())
        for raw, expected in [('0', False), ('1', True)]:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': raw}):
                self.assertEqual(D._aster_exact_memo_enabled(), expected)
        for raw in ['', '2', 'true', ' 1', '1 ', '01', '-1']:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': raw}):
                with self.assertRaisesRegex(ValueError, 'must be 0 or 1'):
                    D._aster_exact_memo_enabled()

    def test_actual_bpe_streams_in_both_cache_modes(self):
        vocab = json.loads((ROOT / 'tokenizer/vocab.json').read_text())
        tokens = [None] * len(vocab)
        for word, i in vocab.items():
            tokens[i] = word
        tools = [dict(name='add_numbers', description='Add two integers.', parameters=dict(
            type='object', properties=dict(a=dict(type='integer'), b=dict(type='integer')), required=['a', 'b']))]
        arguments = dict(a=17, b=25)
        prefix = D.INVOKE_OPEN + ' name="add_numbers">'
        bodies = [json.dumps(arguments), D.P_OPEN + ' name="a" string="false">17' + D.P_CLOSE
                  + D.P_OPEN + ' name="b" string="false">25' + D.P_CLOSE]
        passed = 0
        for mode in ['0', '1']:
            with patch.dict(os.environ, {'ASTER_XQ_EXACT_TOKEN_CACHE': mode}):
                tk = D.Ds41Tokenizer(tokens, (ROOT / 'tokenizer/merges.txt').read_text().split('\n'),
                                      json.loads((ROOT / 'tokenizer/token_type.json').read_text()))
            self.assertEqual(tk._aster_encode_memo is not None, mode == '1')
            for body in bodies:
                for wrap in [False, True]:
                    for forced in [False, True]:
                        for width in [1, 7, 1000, 'bpe']:
                            with self.subTest(mode=mode, body=body, wrap=wrap, forced=forced, width=width):
                                parser = D.F.OutputParser(thinking=False, tools=tools)
                                text = prefix + body + D.INVOKE_CLOSE
                                if wrap:
                                    text = D.CALLS_OPEN + text + D.CALLS_CLOSE
                                events = []
                                if forced:
                                    events += parser.feed(prefix)
                                    text = body + D.INVOKE_CLOSE
                                if width == 'bpe':
                                    decoder = codecs.getincrementaldecoder('utf-8')()
                                    chunks = [decoder.decode(tk.token_bytes(i)) for i in tk.encode(text, parse_special=True)]
                                    chunks.append(decoder.decode(b'', final=True))
                                else:
                                    chunks = [text[i:i + width] for i in range(0, len(text), width)]
                                for chunk in chunks:
                                    events += parser.feed(chunk)
                                events += parser.finish(reason='stop')
                                calls = [dict(name=e.call.name, arguments=e.call.arguments)
                                         for e in events if e.kind == 'tool_call']
                                self.assertEqual(calls, [dict(name='add_numbers', arguments=arguments)])
                                passed += 1
        self.assertEqual(passed, 64)
        print('PASS 64 complete native stream cases, including actual BPE token boundaries and both cache modes')


if __name__ == '__main__':
    unittest.main()
