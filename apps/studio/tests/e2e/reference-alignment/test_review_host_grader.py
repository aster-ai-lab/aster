"""Equivalent source anchors are valid; invented quotes must still fail."""

import importlib.util
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('review_grader', HERE / 'review-host-grader.py')
grader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grader)


class ReviewGraderTests(unittest.TestCase):
    def test_selected_b_requires_actual_b_read_binding_answer_and_grounded_result(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copytree(HERE / 'cases/review-selected-b/inputs', root / 'inputs')
            shutil.copyfile(HERE / 'cases/review-selected-b/tests/expected.json', root / 'expected.json')
            a, b = str(root / 'inputs/a.docx'), str(root / 'inputs/b.docx')
            revision = hashlib.sha256(Path(b).read_bytes()).hexdigest()
            question = json.dumps({'question': '请选择文档', 'choices': [{'id': 'question-0', 'label': a}, {'id': 'question-1', 'label': b}], 'allowFreeText': False})
            binding = {'protocolVersion': 1, 'requestId': 'r1', 'owner': {'sessionId': 's1', 'promptId': 'review-source'}, 'revision': 'sha256:' + hashlib.sha256(question.encode()).hexdigest()}
            finding = {'id': 'b', 'title': '错字', 'evidenceQuote': '以经', 'summary': '错字', 'guidance': '已经'}
            turn = {'status': 'success', 'reviewPublication': {'sha256': {'fixture': 'digest'}},
                    'reviewMemory': {'document': b, 'documentRevision': revision}, 'reviewDocumentReads': [b], 'reviewToolUpdates': [],
                    'reviewForms': [{'type': 'data-form', 'id': 'r1', 'data': {'state': 'form-requested', 'format': 'question', 'binding': binding, 'spec': question}}],
                    'questionReplies': [{'binding': binding, 'values': {'choice': 'question-1', 'answer': ''}}],
                    'messages': [{'role': 'assistant', 'status': 'success', 'data': {'parts': [
                        {'type': 'data-review-source', 'data': {'kind': 'document', 'path': b, 'sha256': revision}},
                        {'type': 'text', 'text': '```a3s-review-finding\n' + json.dumps(finding) + '\n```'}]}}]}
            grader.ROOT = grader.TESTS = root
            path = root / 'turn-result.json'
            path.write_text(json.dumps(turn))
            grader.test_selected_word_review_contract_and_scope()
            for field, invalid in [('reviewDocumentReads', [a, b]), ('reviewDocumentReads', []),
                                   ('reviewMemory', {'document': a}), ('reviewMemory', {'document': b, 'documentRevision': '0'*64}), ('questionReplies', []),
                                   ('reviewToolUpdates', [{'update': {'sessionUpdate': 'tool_call', 'title': 'Read A'}}])]:
                altered = dict(turn, **{field: invalid})
                path.write_text(json.dumps(altered))
                with self.assertRaises(AssertionError):
                    grader.test_selected_word_review_contract_and_scope()

    def test_clarification_wait_refuses_any_silently_selected_document(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            grader.ROOT = grader.TESTS = root
            (root / 'expected.json').write_text(json.dumps({'ambiguous': True}))
            question = {'_meta': {'question': {'allowFreeText': True, 'hasChoices': True}}, 'toolCall': {'title': '请选择一份文件', 'toolCallId': 'q1'}, 'options': [{'optionId': 'question-0', 'name': 'a.docx'}]}
            spec = 'root = Card("Choose", [])'
            form = {'type': 'data-form', 'id': 'f1', 'data': {'format': 'openui', 'spec': spec, 'state': 'form-requested', 'binding': {'protocolVersion': 1, 'revision': 'sha256:' + hashlib.sha256(spec.encode()).hexdigest(), 'requestId': 'f1', 'owner': {'sessionId': 's1'}}}}
            question_form = json.loads(json.dumps(form))
            spec = json.dumps({'question': '请选择一份文件', 'choices': [{'id': 'question-0', 'label': 'a.docx'}, {'id': 'question-1', 'label': 'b.docx'}], 'allowFreeText': True})
            question_form['data'].update(format='question', spec=spec)
            question_form['data']['binding']['revision'] = 'sha256:' + hashlib.sha256(spec.encode()).hexdigest()
            for field, value in [('reviewQuestions', [question]), ('reviewForms', [form]), ('reviewForms', [question_form])]:
                turn = {'status': 'requires-input', 'reviewPublication': {'sha256': {'fixture': 'digest'}},
                        'reviewMemory': {'document': '', 'findings': ''}, field: value,
                        'messages': [{'role': 'assistant', 'status': 'paused', 'data': {'parts': []}}]}
                path = root / 'turn-result.json'
                path.write_text(json.dumps(turn))
                grader.test_selected_word_review_contract_and_scope()
                turn['reviewMemory']['document'] = '/unselected/a.docx'
                path.write_text(json.dumps(turn))
                with self.assertRaises(AssertionError):
                    grader.test_selected_word_review_contract_and_scope()
                turn['reviewMemory']['document'] = ''
                turn['status'] = 'success'
                path.write_text(json.dumps(turn))
                with self.assertRaises(AssertionError):
                    grader.test_selected_word_review_contract_and_scope()

    def test_notice_body_is_valid_evidence_of_a_report_title_conflict(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'inputs').mkdir()
            shutil.copyfile(HERE / 'cases/review-mixed/inputs/notice.docx', root / 'inputs/notice.docx')
            document = str(root / 'inputs/notice.docx')
            revision = hashlib.sha256(Path(document).read_bytes()).hexdigest()
            finding = {'id': '1', 'title': '文种', 'evidenceQuote': '请各处室按照本通知组织政务平台测试',
                       'summary': '标题为报告，正文为通知。', 'guidance': '应核实后统一为通知。'}
            (root / 'expected.json').write_text(json.dumps({'required': [[['报告', '本通知'], '通知']], 'excluded': [], 'ambiguous': False}))
            def write_turn():
                text = '```a3s-review-finding\n' + json.dumps(finding) + '\n```'
                (root / 'turn-result.json').write_text(json.dumps({'status': 'success', 'reviewPublication': {'sha256': {'fixture': 'digest'}},
                    'reviewMemory': {'document': document, 'documentRevision': revision}, 'reviewDocumentReads': [document],
                    'messages': [{'role': 'assistant', 'status': 'success', 'data': {'parts': [
                        {'type': 'data-review-source', 'data': {'kind': 'document', 'path': document, 'sha256': revision}},
                        {'type': 'text', 'text': text}]}}]}))
            grader.ROOT = grader.TESTS = root
            write_turn()
            grader.test_selected_word_review_contract_and_scope()
            finding['evidenceQuote'] = '原文不存在的本通知'
            write_turn()
            with self.assertRaises(AssertionError):
                grader.test_selected_word_review_contract_and_scope()
