"""Conservative multi-signal stall policy; no dependence on GPU or client libraries."""
class StallWindow:
    def __init__(self,seconds=90,max_gap=30):
        if seconds<60 or max_gap<=0:raise ValueError('bounded stall window required')
        self.seconds=seconds;self.max_gap=max_gap;self.base=None;self.last=None

    def observe(self,s):
        keys=('now','identity','active','gpu_idle','cpu_seconds','io_bytes','progress')
        if set(s)!=set(keys):raise ValueError('exact observation schema required')
        for k in ('now','cpu_seconds','io_bytes'):
            if isinstance(s[k],bool) or not isinstance(s[k],(int,float)) or not 0<=s[k]<1e20:raise ValueError('finite nonnegative counter required')
        if not isinstance(s['active'],bool) or not isinstance(s['gpu_idle'],bool):raise ValueError('explicit activity booleans required')
        if not s['active'] or not s['gpu_idle']:
            self.base=self.last=None;return False
        p=self.last
        reset=(p is None or s['identity']!=p['identity'] or s['now']<=p['now'] or s['now']-p['now']>self.max_gap or
               s['cpu_seconds']<p['cpu_seconds'] or s['io_bytes']<p['io_bytes'] or s['progress']!=p['progress'])
        if reset:self.base=dict(s)
        b=self.base
        if s['cpu_seconds']-b['cpu_seconds']>0.5 or s['io_bytes']-b['io_bytes']>1048576:self.base=dict(s)
        self.last=dict(s)
        return s['now']-self.base['now']>=self.seconds

    def reset(self):
        self.base=self.last=None
