"""Original 120-second felt-key score and a restrained shared-room UI palette."""
import json
from pathlib import Path
import numpy as np
from scipy import signal
from sfx_palette import SR, Score, air, wood, sub

ROOT = Path(__file__).resolve().parent
data = json.loads((ROOT / "events.json").read_text())
score = Score(dur=data["T"], T60=.48)
for e in data["events"]:
    t, kind, pan, v = (e[k] for k in ("t", "kind", "pan", "v"))
    if kind == "run":
        score.place(wood(185, .12), t, .3*v, pan, .16)
    elif kind == "open":
        score.place(air(.45, 380, 1900, 1.3, .4), t, .24*v, pan, .24, -.13)
    elif kind == "focus":
        score.place(air(.3, 650, 1500, 1.2, .3), t, .14*v, pan, .2, -.2)
    elif kind == "fold":
        score.place(air(.62, 1700, 180, 1.2, .4), t, .3*v, pan, .3, -.25)
    elif kind == "seal":
        score.place(sub(82, .4), t, .23*v, pan, .2)
        score.place(wood(160, .12), t, .3*v, pan, .15)

def felt(f, d=1.35):
    t = np.arange(int(SR*d))/SR
    tone = np.sin(2*np.pi*f*t)+.2*np.sin(2*np.pi*f*2.002*t)+.06*np.sin(2*np.pi*f*4.01*t)
    env = (1-np.exp(-100*t))*np.exp(-3.8*t)*np.minimum(1, np.maximum(0, d-t)/.1)
    return signal.sosfilt(signal.butter(2, 3300, fs=SR, output="sos"), tone*env)

# Each chapter answers the opening motif; empty intervals leave reading space.
passages = [
 (0, [.4,1.8,3.1,5.4,8.6,10.1], [60,67,64,72,69,67]),
 (13, [.3,2.1,4.5,6.2,8.9,11.4], [64,67,72,76,72,67]),
 (26, [.5,2.8,5.1,7.7,10.3,13.1], [62,69,65,72,69,65]),
 (43, [.4,2.1,4.9,7.8,10.1,13.3], [60,67,72,76,72,69]),
 (62, [.4,2.7,5.4,8.8,11.4], [65,69,72,77,74]),
 (76, [.3,1.8,4.9,7.1,10.4], [62,69,74,72,69]),
 (89, [.4,2.8,5.6,8.1,10.4,12.8], [64,67,72,76,74,72]),
 (105, [.4,2.1,4.5,6.9,9.2,11.2], [60,67,64,72,69,60]),
]
for start, times, notes in passages:
    for i, (dt, n) in enumerate(zip(times, notes)):
        score.place(felt(440*2**((n-69)/12)), start+dt, .075, -.14 if i%2 else .14, .24)

def pad(f, d):
    t = np.arange(int(SR*d))/SR
    env = np.minimum(t/1.1, 1)*np.minimum(np.maximum(d-t, 0)/1.3, 1)
    return (np.sin(2*np.pi*f*t)+.15*np.sin(2*np.pi*f*2*t))*env*.12

for t, n, d in [(5,48,3),(14,52,3),(23,50,3),(32,53,3),(44,48,3),
                (53,52,3),(64,53,3),(72,57,3),(78,50,3),(90,53,3),
                (100,52,3),(110,48,3)]:
    score.place(pad(440*2**((n-69)/12), d), t, .075, 0, .22)
score.write(str(ROOT / "score.wav"), peak_db=-5)
(ROOT / "SOUND-CREDITS.md").write_text("# Sound credits\n\nOriginal 120-second ToWork score, authored for this film. No commercial recording, voice or reference-film audio was sampled. Synthetic wood, air and sub effects share one room. The CSV reading hold and final lockup include intentional silence. Palette code licence: ONETAKE-LICENSE.txt.\n")
