"""Exact public full-window long-output fixture and closed-form correctness checks."""
import hashlib
import json
import math

CONTEXT = 1048576
OUTPUT = 1024
SLACK = 8
INPUT = CONTEXT - OUTPUT - SLACK
EXPECTED = dict(begin=731927, middle=408563, end=962814)


def strict_object(pairs):
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError('duplicate JSON key')
        result[name] = value
    return result


def refuse_constant(value):
    raise ValueError('nonfinite JSON constant refused')


def check_record_prefix(content):
    text = content.lstrip()
    value, end = json.JSONDecoder(object_pairs_hook=strict_object,
                                  parse_constant=refuse_constant).raw_decode(text)
    if (type(value) is not dict or value.keys() != EXPECTED.keys()
            or any(type(value[key]) is not int or value[key] != EXPECTED[key] for key in EXPECTED)):
        raise ValueError('full-window begin/middle/end record values differ')
    if end >= len(text) or text[end] not in '\r\n':
        raise ValueError('record JSON must be followed by a newline and long answer')
    if len(text[end:].strip()) < 1000:
        raise ValueError('record answer has no substantive long continuation')
    return value


def build_fixture(tokenizer, template):
    def render(text):
        return template.render([dict(role='user', content=text)], enable_thinking=False)

    def ids(text):
        return tokenizer.encode(render(text), parse_special=True)

    prefix = ('Public full-window long-output test. Read the three record codes. '
              'Ignore all filler words. Follow the final answer instructions.\n'
              'BEGIN_RECORD_CODE: 731927\nFiller:')
    middle = '\nMIDDLE_RECORD_CODE: 408563\nFiller:'
    suffix = (
        '\nEND_RECORD_CODE: 962814\n'
        'On the first line return only the three record codes as a plain JSON object '
        'with keys begin, middle, end and integer values. Do not use markdown fences. '
        'After that line, write a detailed technical chapter of at least 2500 words '
        'explaining cache consistency in distributed systems. Cover invalidation, TTL, '
        'write-through and write-back, replication, failover, race conditions, versioning, '
        'concrete timelines, pseudocode, monitoring and operational trade-offs. '
        'Explain every topic in detail with multiple examples. Do not repeat filler words.'
    )
    filler_count = INPUT - len(ids(prefix + middle + suffix))
    for _ in range(8):
        if filler_count < 0:
            raise RuntimeError('full-window public fixture overhead exceeds input bound')
        text = prefix + ' cat' * (filler_count // 2) + middle + ' cat' * (filler_count - filler_count // 2) + suffix
        token_ids = ids(text)
        if len(token_ids) == INPUT:
            break
        filler_count += INPUT - len(token_ids)
    else:
        raise RuntimeError('exact full-window long-output token accounting failed')
    rendered = render(text)
    positions = {name: len(tokenizer.encode(rendered[:rendered.index(marker)], parse_special=True))
                 for name, marker in [('begin', 'BEGIN_RECORD_CODE:'),
                                      ('middle', 'MIDDLE_RECORD_CODE:'),
                                      ('end', 'END_RECORD_CODE:')]}
    if not (positions['begin'] < 128 and abs(positions['middle'] - INPUT // 2) < 256
            and positions['end'] > INPUT - 512):
        raise RuntimeError('full-window public record placement differs')
    raw = text.encode()
    id_bytes = b''.join(int(value).to_bytes(4, 'little') for value in token_ids)
    return text, dict(context_tokens=CONTEXT, actual_prompt_tokens=INPUT,
                      max_output_tokens=OUTPUT, context_safety_tokens=SLACK,
                      total_reserved_tokens=INPUT + OUTPUT + SLACK,
                      prompt_sha256=hashlib.sha256(raw).hexdigest(), prompt_bytes=len(raw),
                      prompt_id_sha256=hashlib.sha256(id_bytes).hexdigest(),
                      record_token_positions=positions, expected=EXPECTED)


def check_completion(row, metadata, *, cold, reference_sha256=None):
    if (metadata['context_tokens'] != CONTEXT or metadata['actual_prompt_tokens'] != INPUT
            or metadata['max_output_tokens'] != OUTPUT or metadata['context_safety_tokens'] != SLACK
            or metadata['total_reserved_tokens'] != CONTEXT):
        raise ValueError('exact full-window input/output reservation differs')
    usage = row.get('usage') or {}
    if (row.get('status') != 200 or not row.get('done') or row.get('finish_reason') != 'length'
            or usage.get('prompt_tokens') != INPUT or usage.get('completion_tokens') != OUTPUT
            or usage.get('total_tokens') != INPUT + OUTPUT):
        raise ValueError('actual full-window long-output request did not finish exactly')
    cached = usage.get('prompt_tokens_details', {}).get('cached_tokens')
    if type(cached) is not int or cached > INPUT or (cached != 0 if cold else cached < INPUT - SLACK):
        raise ValueError('actual full-window cache accounting differs')
    if any(type(row.get(name)) not in (int, float) or not math.isfinite(row[name]) or row[name] <= 0
           for name in ['seconds', 'ttft_seconds', 'first_to_last_text_seconds']):
        raise ValueError('complete full-window streaming timing is required')
    check_record_prefix(row['content'])
    digest = hashlib.sha256(row['content'].encode()).hexdigest()
    if row.get('output_sha256') != digest or (reference_sha256 is not None and digest != reference_sha256):
        raise ValueError('full-window long-output identity differs')
    return digest
