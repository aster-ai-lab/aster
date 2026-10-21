"""Original ToWork score, with deliberate space under reading holds."""
import json
from pathlib import Path
import numpy as np
from scipy import signal
from sfx_palette import SR, Score, air, glass, wood, sub

ROOT = Path(__file__).resolve().parent
data = json.loads((ROOT / "events.json").read_text())
score = Score(dur=data["T"], T60=.55)
for e in data["events"]:
    t, kind, pan, v = (e[k] for k in ("t", "kind", "pan", "v"))
    if kind == "key": score.place(wood(240,.07),t,.16*v,pan,.1)
    elif kind == "run": score.place(wood(180,.12),t,.34*v,pan,.15)
    elif kind == "open": score.place(air(.42,420,2200,1.3,.4),t,.25*v,pan,.24,-.12)
    elif kind == "focus": score.place(air(.3,700,1600,1.2,.3),t,.15*v,pan,.2,-.2)
    elif kind == "fold": score.place(air(.6,2000,180,1.2,.4),t,.3*v,pan,.3,-.25)
    elif kind == "seal":
        score.place(sub(82,.4),t,.23*v,pan,.2)
        score.place(wood(160,.12),t,.32*v,pan,.15)

def felt(f,d):
    t = np.arange(int(SR*d))/SR
    tone = np.sin(2*np.pi*f*t)+.22*np.sin(2*np.pi*f*2.003*t)+.07*np.sin(2*np.pi*f*4.01*t)
    env = (1-np.exp(-120*t))*np.exp(-4.5*t)*np.minimum(1,np.maximum(0,d-t)/.08)
    return signal.sosfilt(signal.butter(2,3700,fs=SR,output="sos"),tone*env)

# Five related but separately phrased passages, rather than a repeated short loop.
phrases = [
 (0,[(.2,261.63),(1.4,392),(2.3,329.63),(3.7,523.25),(5.5,392),(7.3,329.63),(9,440),(10.8,392),(13,329.63),(15.8,261.63),(17.8,523.25)]),
 (19,[(.3,329.63),(1.5,392),(3.3,523.25),(8.1,440),(9.2,392),(13.8,349.23),(15.2,329.63),(18.5,261.63),(20.3,392),(22.7,523.25)]),
 (43,[(.1,293.66),(1.3,440),(3.2,349.23),(4.6,523.25),(6.5,440),(7.4,349.23),(10.8,293.66),(15.8,440),(17.1,523.25),(18.3,659.25),(21.6,523.25),(24.6,440)]),
 (69,[(.2,329.63),(1.3,392),(2.8,523.25),(5.5,659.25),(7.2,523.25),(10.6,392),(12.1,349.23),(14.5,440),(16.5,523.25)]),
 (86,[(.6,261.63),(1.5,392),(3.2,523.25),(5.3,440),(7.2,392),(9.3,329.63),(11.2,440),(13.1,523.25),(15.4,392),(18.6,349.23),(20.3,329.63),(21.7,392),(23.6,523.25),(25.2,392),(27.4,261.63)])
]
for start, notes in phrases:
    for i,(t,f) in enumerate(notes): score.place(felt(f,1.05),start+t,.065,(-.12 if i%2 else .12),.25)

def pad(f,d):
    t=np.arange(int(SR*d))/SR
    env=np.minimum(t/.9,1)*np.minimum(np.maximum(d-t,0)/1.3,1)
    return (np.sin(2*np.pi*f*t)+.16*np.sin(2*np.pi*f*2*t))*env*.12
for t,f,d in [(3.8,130.81,4.5),(10.8,164.81,5),(38,130.81,3.8),(43,146.83,5),(59.3,174.61,3.5),(69.5,164.81,4.3),(81,174.61,4),(87,130.81,4),(95,146.83,4),(108.5,130.81,5)]:
    score.place(pad(f,d),t,.07,0,.22)
score.write(str(ROOT / "score.wav"),peak_db=-5)
(ROOT / "SOUND-CREDITS.md").write_text("# Sound credits\n\nOriginal 120-second ToWork score, authored for this film. No commercial recording, voice or reference-film audio was sampled. The onetake synthetic wood, air and sub palette shares one short room and follows composition events. Reading holds and the final lockup leave deliberate silence. Palette code licence: ONETAKE-LICENSE.txt.\n")
