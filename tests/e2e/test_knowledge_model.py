import copy
import json
import unittest

from knowledge_model import CASE, SOURCE, SOURCE_PATH, STEPS, decide


class KnowledgeModelTests(unittest.TestCase):
    def body(self):
        return {'model': CASE, 'tools': [{'function': {'name': tool}} for tool in ['read', 'write']],
                'messages': [{'role': 'user', 'content': 'Host-supplied work order'}]}

    def result(self, index):
        call_id, tool, args = STEPS[index]
        if index == 0:
            content = f'     1\tOperation: ingest. Input source paths: ["{SOURCE_PATH}"]\n     2\t.wiki-changes.json\n'
        elif index == 1:
            content = '\n'.join(f'{n:6}\t{line}' for n, line in enumerate(SOURCE.splitlines(), 1))
        else:
            content = f'Wrote {len(args["content"].encode())} bytes (1 lines) to {args["file_path"]}'
        return {'role': 'tool', 'tool_call_id': call_id, 'content': 'Tool exit code: 0\n' + content}

    def test_only_stages_after_correlated_reads_and_confirmed_writes(self):
        body = self.body()
        for index, (call_id, tool, args) in enumerate(STEPS):
            message, finish, event = decide(body)
            self.assertEqual(finish, 'tool_calls')
            call = message['tool_calls'][0]
            self.assertEqual(call['id'], call_id)
            self.assertEqual(call['function']['name'], tool)
            self.assertEqual(json.loads(call['function']['arguments']), args)
            self.assertEqual(event['completed_tools'], index)
            body['messages'].append({'role': 'assistant', 'tool_calls': [call]})
            body['messages'].append(self.result(index))
        self.assertEqual(decide(body)[0]['content'], 'KNOWLEDGE-BASIC-STAGED')

    def test_correlates_the_hosts_fresh_invocation_identity(self):
        body = self.body()
        call = decide(body)[0]['tool_calls'][0]
        call['id'] = 'host-owned-invocation'
        result = self.result(0)
        result['tool_call_id'] = call['id']
        body['messages'] += [{'role': 'assistant', 'tool_calls': [call]}, result]
        self.assertEqual(decide(body)[2]['completed_tools'], 1)

    def test_no_tool_auxiliary_request_cannot_invent_memory(self):
        body = self.body()
        body['tools'] = []
        self.assertEqual(json.loads(decide(body)[0]['content']), {'items': []})

    def test_unknown_model_missing_tools_and_bad_receipts_fail_closed(self):
        body = self.body()
        variants = [dict(body, model='unknown'), dict(body, tools=[{'function': {'name': 'read'}}])]
        for index in range(len(STEPS)):
            bad = copy.deepcopy(body)
            for i, (call_id, tool, args) in enumerate(STEPS[:index + 1]):
                bad['messages'] += [
                    {'role': 'assistant', 'tool_calls': [{'id': call_id, 'type': 'function',
                        'function': {'name': tool, 'arguments': json.dumps(args)}}]},
                    self.result(i),
                ]
            bad['messages'][-1]['content'] = 'permission denied'
            variants.append(bad)
        variants += [dict(body, messages=[dict(self.result(0), tool_call_id='other')]),
                     dict(body, messages=[self.result(1)]),
                     dict(body, messages=[self.result(i) for i in range(4)] + [self.result(3)])]
        for variant in variants:
            with self.assertRaises(ValueError):
                decide(variant)


if __name__ == '__main__':
    unittest.main()
