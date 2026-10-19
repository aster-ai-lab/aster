"""Assert that the actual model receives the explicit answer, not a default choice."""

import json
from pathlib import Path

TESTS = Path('/tests')

def test_native_question_answer_reaches_the_real_model():
    turn = json.loads((TESTS / 'turn-result.json').read_text())
    expected = json.loads((TESTS / 'expected.json').read_text())
    assert turn['status'] == 'success'
    forms, replies = turn['reviewForms'], turn['questionReplies']
    assert len(forms) == len(replies) == 1
    assert forms[0]['data']['format'] == 'question'
    assert forms[0]['data']['binding'] == replies[0]['binding']
    question = json.loads(forms[0]['data']['spec'])
    values = replies[0]['values']
    if expected['kind'] == 'choice':
        selected = [choice for choice in question['choices'] if choice['id'] == values['choice']]
        assert len(selected) == 1 and selected[0]['label'] == expected['answer']
        assert question['choices'][0]['id'] != values['choice'] and not values['answer']
    else:
        assert question['choices'] == [] and question['allowFreeText'] is True
        assert not values['choice'] and values['answer'] == expected['answer']
    messages = [message for message in turn['messages'] if message['role'] == 'assistant']
    assert messages and all(message['status'] == 'success' for message in messages)
    texts = [part['text'] for message in messages for part in message['data']['parts'] if part['type'] == 'text']
    assert texts and texts[-1].strip() == expected['answer'], texts
