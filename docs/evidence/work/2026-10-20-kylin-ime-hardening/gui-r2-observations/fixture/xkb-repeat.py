import ctypes, json, os, sys
from pathlib import Path

root = Path('/aster/target/evaluations/kylin-ime-gui')
scope = Path((root / 'last-run').read_text().strip())
env = json.loads((scope / 'environment.json').read_text())
os.environ.update(env)
lib = ctypes.CDLL('libX11.so.6')
lib.XOpenDisplay.argtypes = [ctypes.c_char_p]
lib.XOpenDisplay.restype = ctypes.c_void_p
lib.XCloseDisplay.argtypes = [ctypes.c_void_p]
lib.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
lib.XAutoRepeatOff.argtypes = [ctypes.c_void_p]
lib.XAutoRepeatOn.argtypes = [ctypes.c_void_p]
lib.XkbGetAutoRepeatRate.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.POINTER(ctypes.c_uint), ctypes.POINTER(ctypes.c_uint)]

class KeyboardState(ctypes.Structure):
    _fields_ = [('key_click_percent', ctypes.c_int), ('bell_percent', ctypes.c_int),
                ('bell_pitch', ctypes.c_uint), ('bell_duration', ctypes.c_uint),
                ('led_mask', ctypes.c_ulong), ('global_auto_repeat', ctypes.c_int),
                ('auto_repeats', ctypes.c_char * 32)]

lib.XGetKeyboardControl.argtypes = [ctypes.c_void_p, ctypes.POINTER(KeyboardState)]
dpy = lib.XOpenDisplay(None)
if not dpy:
    raise SystemExit('Cannot connect to owned display')
try:
    before = KeyboardState()
    lib.XGetKeyboardControl(dpy, ctypes.byref(before))
    action = sys.argv[1] if len(sys.argv) > 1 else 'get'
    if action == 'off':
        lib.XAutoRepeatOff(dpy)
    elif action == 'on':
        lib.XAutoRepeatOn(dpy)
    elif action != 'get':
        raise SystemExit('Expected get/off/on')
    lib.XSync(dpy, 0)
    after = KeyboardState()
    lib.XGetKeyboardControl(dpy, ctypes.byref(after))
    delay, interval = ctypes.c_uint(), ctypes.c_uint()
    ok = lib.XkbGetAutoRepeatRate(dpy, 0x100, ctypes.byref(delay), ctypes.byref(interval))
    record = {'action': action, 'display': env['DISPLAY'], 'before_global_repeat': before.global_auto_repeat,
              'after_global_repeat': after.global_auto_repeat, 'xkb_rate_available': bool(ok),
              'delay_ms': delay.value, 'interval_ms': interval.value}
    with (scope / 'xkb-repeat.jsonl').open('a') as out:
        out.write(json.dumps(record) + '\n')
    print(json.dumps(record))
finally:
    lib.XCloseDisplay(dpy)
