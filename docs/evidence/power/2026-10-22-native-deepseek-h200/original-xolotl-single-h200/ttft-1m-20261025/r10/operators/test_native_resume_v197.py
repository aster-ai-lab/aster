import unittest
from aster_native_resume_v197 import validate_resume
def done(reused=0):return 'DONE 128 1047544 1959574.1 8780.0 length 0 0 '+str(reused)+' 39198 40080 0 0 0 1047544'
class Resume(unittest.TestCase):
    def test_cold_native_omits_zero_marker(self):self.assertEqual(validate_resume(['PP 1047544 1047544 1 1'],done(),0),0)
    def test_each_exact_restored_generation(self):
        for index in [1,2,3]:self.assertEqual(validate_resume(['RESUME 1047544'],done(1047544),index),1047544)
    def test_missing_positive_and_spurious_zero(self):
        for lines,text,index in [([],done(1047544),1),(['RESUME 0'],done(),0)]:
            with self.assertRaises(ValueError):validate_resume(lines,text,index)
    def test_duplicate_or_noncanonical_marker(self):
        for lines in [['RESUME 1047544']*2,['RESUME  1047544'],['RESUME 1047543']]:
            with self.assertRaises(ValueError):validate_resume(lines,done(1047544),1)
    def test_DONE_reuse_independent(self):
        for lines,text,index in [([],done(1),0),(['RESUME 1047544'],done(0),1)]:
            with self.assertRaises(ValueError):validate_resume(lines,text,index)
    def test_wrong_completion_or_ordinal(self):
        for text,index in [('DONE 127 1047544 1 1 length 0 0 0',0),('DONE 128 1047543 1 1 length 0 0 0',0),(done(),4),(done(),True)]:
            with self.assertRaises(ValueError):validate_resume([],text,index)
if __name__=='__main__':unittest.main()
