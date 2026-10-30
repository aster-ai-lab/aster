import unittest
from aster_stall_window_v415 import StallWindow

def sample(t,**kw):
    return dict(now=t,identity=(314,100,316,101),active=True,gpu_idle=True,cpu_seconds=1,io_bytes=1,progress=(0,None),**kw)

class StallPolicy(unittest.TestCase):
    def run_window(self,change):
        w=StallWindow();results=[]
        for t in range(0,101,10):
            s=sample(t);change(s,t);results.append(w.observe(s))
        return results
    def test_complete_inactivity_requires_full_window(self):
        r=self.run_window(lambda s,t:None);self.assertFalse(any(r[:9]));self.assertTrue(r[9])
    def test_gpu_work_is_never_killed(self):self.assertFalse(any(self.run_window(lambda s,t:s.update(gpu_idle=False))))
    def test_cpu_work_is_never_killed(self):self.assertFalse(any(self.run_window(lambda s,t:s.update(cpu_seconds=t))))
    def test_cached_io_work_is_never_killed(self):self.assertFalse(any(self.run_window(lambda s,t:s.update(io_bytes=t*1048576))))
    def test_prompt_progress_is_never_killed(self):self.assertFalse(any(self.run_window(lambda s,t:s.update(progress=(0,t)))))
    def test_idle_service_is_never_killed(self):self.assertFalse(any(self.run_window(lambda s,t:s.update(active=False))))
    def test_native_generation_change_resets_window(self):
        r=self.run_window(lambda s,t:s.update(identity=(314,100,t//50,101)));self.assertFalse(any(r))
    def test_telemetry_gap_resets_window(self):
        w=StallWindow();self.assertFalse(w.observe(sample(0)));self.assertFalse(w.observe(sample(91)))
    def test_counter_regression_resets_window(self):
        w=StallWindow()
        for t in range(0,90,10):self.assertFalse(w.observe(sample(t)))
        s=sample(90);s['cpu_seconds']=0;self.assertFalse(w.observe(s))
    def test_missing_or_nonfinite_telemetry_refused(self):
        w=StallWindow();s=sample(0);del s['gpu_idle']
        with self.assertRaises(ValueError):w.observe(s)
        for value in [float('nan'),float('inf'),-1]:
            s=sample(0);s['cpu_seconds']=value
            with self.assertRaises(ValueError):w.observe(s)

if __name__=='__main__':unittest.main()
