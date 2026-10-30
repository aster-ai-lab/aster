import copy
import json
import unittest

from json_render_model import decide, EXPECTED


class ModelOracleTests(unittest.TestCase):
    def body(self, content=None, case='SUBMIT', call_id=None):
        messages = [{'role': 'user', 'content': 'JSON-RENDER-CUA-' + case}]
        if content is not None:
            messages.append({'role': 'tool', 'tool_call_id': call_id or 'call-json-render-cua-' + case.lower(),
                             'content': content})
        return {'messages': messages, 'tools': [{'function': {'name': 'ask_user_form'}}]}

    def test_calls_real_tool_with_shared_json_fixture(self):
        message, finish, event = decide(self.body())
        self.assertEqual(finish, 'tool_calls')
        call = message['tool_calls'][0]
        self.assertEqual(call['function']['name'], 'ask_user_form')
        form = json.loads(json.loads(call['function']['arguments'])['form'])
        self.assertEqual(form['state']['form']['amount'], 3)
        self.assertEqual(form['elements']['name']['props']['value'], {'$bindState': '/form/name'})
        self.assertTrue(event['offered'])

    def test_accepts_only_typed_correlated_non_permission_receipt(self):
        result = {'schema': 'a3s.code.user-form.v1', 'values': EXPECTED, 'permission_grant': False}
        self.assertTrue(decide(self.body(json.dumps(result)))[2]['accepted'])
        for key, value in [('amount', '4'), ('ready', 1), ('choice', 'beta')]:
            bad = copy.deepcopy(result)
            bad['values'][key] = value
            self.assertFalse(decide(self.body(json.dumps(bad)))[2]['accepted'])
        bad = dict(result, permission_grant=True)
        self.assertFalse(decide(self.body(json.dumps(bad)))[2]['accepted'])
        self.assertEqual(decide(self.body(json.dumps(result), call_id='wrong-call'))[1], 'tool_calls')

    def test_rejects_invalid_receipt_shapes(self):
        for content in ['null', '[]', 'not-json', '{}']:
            self.assertFalse(decide(self.body(content))[2]['accepted'])

    def test_latest_user_marker_selects_new_case(self):
        body = self.body(case='INVALID')
        body['messages'].insert(0, {'role': 'user', 'content': 'JSON-RENDER-CUA-SUBMIT'})
        self.assertEqual(decide(body)[2]['case'], 'invalid')
        self.assertNotIn('PASS', decide(self.body('invalid JSON Render document: bad', 'INVALID'))[0]['content'])
        self.assertTrue(decide(self.body('invalid JSON Render document: bad', 'INVALID'))[2]['rejected'])

    def test_requires_tool_and_unambiguous_marker(self):
        body = self.body()
        body['tools'] = []
        self.assertEqual(decide(body)[0]['content'], 'JSON-RENDER-CUA-MISSING-TOOL')
        body['messages'][0]['content'] = 'JSON-RENDER-CUA-SUBMIT JSON-RENDER-CUA-CANCEL'
        self.assertEqual(decide(body)[0]['content'], 'JSON-RENDER-CUA-MISSING-MARKER')

    def test_cancellation_cannot_pass_as_an_answer(self):
        message, _, event = decide(self.body('form was cancelled by the user', 'CANCEL'))
        self.assertEqual(message['content'], 'JSON-RENDER-CUA-CANCELLED')
        self.assertTrue(event['cancelled'])
        self.assertFalse(decide(self.body('some other error', 'CANCEL'))[2]['cancelled'])


if __name__ == '__main__':
    unittest.main()
