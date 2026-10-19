"""Source-bound checks for the actual Word review host, after the agent VM closes."""

import json
import hashlib
from pathlib import Path
import xml.etree.ElementTree as ET
from zipfile import ZipFile

ROOT = Path('/app')
TESTS = Path('/tests')


def quote_matches(anchors, quote):
    return any(anchor in quote for anchor in ([anchors] if isinstance(anchors, str) else anchors))


def test_selected_word_review_contract_and_scope():
    turn = json.loads((TESTS / 'turn-result.json').read_text())
    expected = json.loads((TESTS / 'expected.json').read_text())
    assert turn['reviewPublication']['sha256']
    messages = [m for m in turn['messages'] if m['role'] == 'assistant']
    assert messages
    text = ''.join(p.get('text', '') for m in messages for p in m['data']['parts'] if p['type'] == 'text').strip()
    if expected['ambiguous']:
        assert '```a3s-review-finding' not in text and text != '[]'
        questions = turn.get('reviewQuestions', [])
        forms = turn.get('reviewForms', [])
        assert turn['status'] == 'requires-input' and not turn.get('questionReplies')
        assert (questions or forms) and turn['reviewMemory'] == {'document': '', 'findings': ''}
        assert all(isinstance(q['_meta']['question']['allowFreeText'], bool)
                   and isinstance(q['_meta']['question']['hasChoices'], bool)
                   and q['toolCall']['title'].strip() and q['options'] for q in questions)
        for form in forms:
            data = form['data']
            assert form['type'] == 'data-form' and data['format'] in ('openui', 'question')
            assert data['state'] == 'form-requested' and not data.get('values')
            binding = data['binding']
            assert binding['requestId'] == form['id'] and binding['owner']['sessionId']
            assert binding['protocolVersion'] == 1
            assert binding['revision'] == 'sha256:' + hashlib.sha256(data['spec'].encode()).hexdigest()
            if data['format'] == 'question':
                question = json.loads(data['spec'])
                assert set(question) == {'question', 'choices', 'allowFreeText'}
                assert question['question'].strip() and isinstance(question['allowFreeText'], bool)
                choices = question['choices']
                assert isinstance(choices, list) and len(choices) <= 8
                assert len({choice['id'] for choice in choices}) == len(choices)
                assert all(set(choice) == {'id', 'label'} and choice['id'] and choice['label'].strip() for choice in choices)
                assert choices or question['allowFreeText']
        question_ids = {q['toolCall']['toolCallId'] for q in questions}
        assert not any('tool' in p['type'] and p.get('toolCallId') not in question_ids
                       and not (forms and p.get('toolName') == 'ask_user_form')
                       and not (p.get('toolName') == 'ls' and p.get('input', {}).get('path') == 'inputs')
                       for m in messages for p in m['data']['parts'])
        return
    assert turn['status'] == 'success' and all(m['status'] == 'success' for m in messages)
    if expected.get('source') == 'text':
        source = (ROOT / 'inputs/source.txt').read_text()
        revision = hashlib.sha256(source.encode()).hexdigest()
        assert turn['reviewMemory']['document'] == ''
        assert turn['reviewMemory']['textRevision'] == revision
        sources = [p['data'] for m in turn['messages'] if m['role'] == 'user'
                   for p in m['data']['parts'] if p['type'] == 'data-review-source']
        assert sources == [{'kind': 'text', 'text': source, 'sha256': revision}]
    else:
        documents = list((ROOT / 'inputs').glob('*.docx'))
        if expected.get('selected'):
            assert len(documents) == 2
            forms, replies = turn['reviewForms'], turn['questionReplies']
            assert len(forms) == len(replies) == 1
            data = forms[0]['data']
            assert data['format'] == 'question' and data['state'] == 'form-requested'
            binding = data['binding']
            assert binding == replies[0]['binding'] and binding['owner']['promptId'] == 'review-source'
            assert binding['requestId'] == forms[0]['id'] and binding['protocolVersion'] == 1
            assert binding['revision'] == 'sha256:' + hashlib.sha256(data['spec'].encode()).hexdigest()
            question = json.loads(data['spec'])
            assert question['allowFreeText'] is False and len(question['choices']) == 2
            selected = question['choices'][1]
            assert selected['label'].endswith('/inputs/' + expected['selected'])
            assert question['choices'][0]['label'].endswith('/inputs/' + expected['unselected'])
            assert replies[0]['values'] == {'choice': selected['id'], 'answer': ''}
            assert turn['reviewDocumentReads'] == [selected['label']]
            assert turn['reviewToolUpdates'] == []
            assert turn['reviewMemory']['document'] == selected['label']
            document = ROOT / 'inputs' / expected['selected']
        else:
            assert len(documents) == 1
            document = documents[0]
        revision = hashlib.sha256(document.read_bytes()).hexdigest()
        assert turn['reviewMemory']['documentRevision'] == revision
        bound_path = turn['reviewMemory']['document']
        assert bound_path.endswith('/inputs/' + document.name)
        assert turn['reviewDocumentReads'] == [bound_path]
        sources = [p['data'] for m in messages for p in m['data']['parts'] if p['type'] == 'data-review-source']
        assert sources == [{'kind': 'document', 'path': bound_path, 'sha256': revision}]
        with ZipFile(document) as archive:
            tree = ET.fromstring(archive.read('word/document.xml'))
        ns = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
        source = '\n'.join(''.join(p.itertext()) for p in tree.findall('.//w:p', ns))
    if not expected['required']:
        assert text == '[]', text
        return
    findings = []
    rest = text
    while rest:
        header, body = rest.split('\n', 1)
        assert header == '```a3s-review-finding'
        payload, rest = body.split('\n```', 1)
        finding = json.loads(payload)
        assert set(finding) == {'id', 'title', 'evidenceQuote', 'summary', 'guidance'}
        assert all(isinstance(v, str) and v.strip() for v in finding.values())
        assert finding['evidenceQuote'] in source
        assert finding['guidance'].strip() != finding['evidenceQuote'].strip()
        findings.append(finding)
        rest = rest.strip()
    assert findings
    keys = [(f['evidenceQuote'], f['title'], f['guidance']) for f in findings]
    assert len(keys) == len(set(keys))
    for quote, correction in expected['required']:
        assert any(quote_matches(quote, f['evidenceQuote']) and correction in f['guidance'] for f in findings), (quote, findings)
    for phrase in expected['excluded']:
        assert not any(phrase in f['evidenceQuote'] and not any(quote_matches(q, f['evidenceQuote']) for q, _ in expected['required']) for f in findings), (phrase, findings)
