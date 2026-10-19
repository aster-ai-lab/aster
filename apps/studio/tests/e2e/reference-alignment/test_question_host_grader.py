"""A model echo cannot replace evidence of the actual explicit choice."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('question_grader', HERE / 'question-host-grader.py')
grader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grader)


class QuestionGraderTests(unittest.TestCase):
    def test_a_correct_echo_with_the_wrong_selected_choice_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            grader.TESTS = root
            (root / 'expected.json').write_text(json.dumps({'kind': 'choice', 'answer': 'B.docx'}))
            question = {'question': 'Which?', 'allowFreeText': False, 'choices': [{'id': 'q0', 'label': 'A.docx'}, {'id': 'q1', 'label': 'B.docx'}]}
            turn = {'status': 'success', 'reviewForms': [{'data': {'format': 'question', 'binding': {'id': 'binding'}, 'spec': json.dumps(question)}}],
                    'questionReplies': [{'binding': {'id': 'binding'}, 'values': {'choice': 'q1', 'answer': ''}}],
                    'messages': [{'role': 'assistant', 'status': 'success', 'data': {'parts': [{'type': 'text', 'text': 'Which document?'}, {'type': 'text', 'text': 'B.docx'}]}}]}
            path = root / 'turn-result.json'
            path.write_text(json.dumps(turn))
            grader.test_native_question_answer_reaches_the_real_model()
            turn['questionReplies'][0]['values']['choice'] = 'q0'
            path.write_text(json.dumps(turn))
            with self.assertRaises(AssertionError):
                grader.test_native_question_answer_reaches_the_real_model()
