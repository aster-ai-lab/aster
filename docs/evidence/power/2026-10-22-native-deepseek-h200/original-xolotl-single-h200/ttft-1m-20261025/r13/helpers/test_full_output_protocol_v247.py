"""The new full-output diagnostic must enforce1024 completions and exact cache reuse."""
import unittest
from aster_native_resume_v197 import validate_resume


class FullOutputDiagnostic(unittest.TestCase):
    def test_cold_and_restored_complete1024_records(self):
        for index in range(4):
            reused=0 if index==0 else 1047544
            done='DONE 1024 1047544 1 1 length 0 0 '+str(reused)
            markers=[] if index==0 else ['RESUME 1047544']
            self.assertEqual(validate_resume(markers,done,index,output_tokens=1024),reused)

    def test_shortened128_output_cannot_satisfy1024(self):
        for index in range(4):
            reused=0 if index==0 else 1047544
            done='DONE 128 1047544 1 1 length 0 0 '+str(reused)
            markers=[] if index==0 else ['RESUME 1047544']
            with self.assertRaises(ValueError):validate_resume(markers,done,index,output_tokens=1024)

    def test_million_input_must_be_actual_count(self):
        for prompt in [4096,131072,1048504,1048576]:
            done='DONE 1024 '+str(prompt)+' 1 1 length 0 0 0'
            with self.assertRaises(ValueError):validate_resume([],done,0,output_tokens=1024)

    def test_restored_full_count_cannot_hide_zero_reuse(self):
        done='DONE 1024 1047544 1 1 length 0 0 0'
        with self.assertRaises(ValueError):validate_resume(['RESUME 1047544'],done,1,output_tokens=1024)


if __name__=='__main__':unittest.main()
