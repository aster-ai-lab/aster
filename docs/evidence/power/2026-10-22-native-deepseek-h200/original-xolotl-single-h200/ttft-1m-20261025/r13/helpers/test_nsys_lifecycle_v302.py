"""Require affirmative successful-empty inventory; never infer absence from a shutdown error."""
import types,unittest
from aster_nsys_lifecycle_v302 import require_no_active_sessions

class NsysLifecycleTests(unittest.TestCase):
    def result(self,rc=0,out='',err=''):
        return types.SimpleNamespace(returncode=rc,stdout=out,stderr=err)
    def test_waited_primary_empty_inventory(self):
        self.assertTrue(require_no_active_sessions(self.result()))
        self.assertTrue(require_no_active_sessions(self.result(out='\n')))
    def test_error_is_not_absence(self):
        for rc in [1,-15,True,'0']:
            with self.subTest(rc=rc),self.assertRaises(ValueError):require_no_active_sessions(self.result(rc=rc))
    def test_an_active_session_refuses_next_launch(self):
        with self.assertRaisesRegex(ValueError,'remains'):require_no_active_sessions(self.result(out='14 2026-10-25 active 0 own-session\n'))
    def test_header_is_not_an_empty_inventory(self):
        with self.assertRaises(ValueError):require_no_active_sessions(self.result(out='ID TIME STATE LAUNCH NAME\n'))
    def test_diagnostic_must_not_be_silenced(self):
        with self.assertRaises(ValueError):require_no_active_sessions(self.result(err='unexpected stderr'))
    def test_nondocumented_or_oversized_payload_refused(self):
        for value in [self.result(out=b''),self.result(err=None),self.result(out='x'*8193),self.result(err='x'*4097)]:
            with self.subTest(value=value),self.assertRaises(ValueError):require_no_active_sessions(value)

if __name__=='__main__':unittest.main(verbosity=2)
