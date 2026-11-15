#!/usr/bin/env python3
"""Record gamepad axis and button events for a while, then summarise them
(Linux, evdev).

Usage: scripts/diag/padrec.py SECONDS [DEVICE] [LABEL]

Move the sticks and press buttons while it runs. For every axis it prints
how many distinct values were seen (analog: many, digital: only 0/127/255)
and the last value; for buttons, how often each was pressed (numbered like
on the Art AG-16T, 1 = first button). Only the Python standard library is
used.
"""
import glob
import os
import select
import struct
import sys
import time
from collections import Counter, defaultdict

NAMES = {0: "X", 1: "Y", 2: "Z", 3: "RX", 4: "RY", 5: "RZ", 16: "HAT0X", 17: "HAT0Y"}
EV_KEY, EV_ABS, BTN_JOYSTICK = 1, 3, 0x120
EVENT = struct.Struct("qqHHi")  # struct input_event on 64-bit Linux

if len(sys.argv) < 2:
    sys.exit(__doc__)
secs = float(sys.argv[1])
if len(sys.argv) > 2:
    path = sys.argv[2]
else:
    found = glob.glob("/dev/input/by-id/*DragonRise*event-joystick")
    if not found:
        sys.exit("no DragonRise pad found; pass the event device as the second argument")
    path = found[0]
label = sys.argv[3] if len(sys.argv) > 3 else path

fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
vals = defaultdict(Counter)
last = {}
presses = Counter()
print(f"Recording for {secs:g} s — move the sticks, press buttons…", file=sys.stderr)
end = time.time() + secs
while time.time() < end:
    ready, _, _ = select.select([fd], [], [], 0.1)
    if not ready:
        continue
    data = os.read(fd, EVENT.size * 64)
    for i in range(0, len(data) - EVENT.size + 1, EVENT.size):
        _, _, typ, code, val = EVENT.unpack_from(data, i)
        if typ == EV_ABS and code in NAMES:
            vals[NAMES[code]][val] += 1
            last[NAMES[code]] = val
        elif typ == EV_KEY and val == 1:
            presses[code - BTN_JOYSTICK + 1] += 1
print(f"== {label}")
for name in NAMES.values():
    c = vals.get(name)
    if not c:
        continue
    distinct = sorted(c)
    sample = distinct if len(distinct) <= 8 else distinct[:4] + ["…"] + distinct[-4:]
    print(f"  {name:6} distinct={len(distinct):3} values={sample} last={last[name]}")
print("  buttons pressed (number: count):", dict(sorted(presses.items())))
