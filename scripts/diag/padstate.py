#!/usr/bin/env python3
"""Print the resting axis values and held buttons of a gamepad (Linux, evdev).

Usage: scripts/diag/padstate.py [DEVICE] [LABEL]

DEVICE defaults to the first DragonRise joystick in /dev/input/by-id. The
pad does not have to move: the current values are read with EVIOCGABS, so
this shows e.g. analog (axes at 128) vs digital mode (axes at 127) on the
Art AG-16T. Only the Python standard library is used.
"""
import fcntl
import glob
import os
import struct
import sys
import time

NAMES = {0: "X", 1: "Y", 2: "Z", 3: "RX", 4: "RY", 5: "RZ", 16: "HAT0X", 17: "HAT0Y"}
ABSINFO = struct.calcsize("6i")


def eviocgabs(code):
    return (2 << 30) | (ABSINFO << 16) | (ord("E") << 8) | (0x40 + code)


def eviocgkey(length):
    return (2 << 30) | (length << 16) | (ord("E") << 8) | 0x18


def default_device():
    found = glob.glob("/dev/input/by-id/*DragonRise*event-joystick")
    if not found:
        sys.exit("no DragonRise pad found; pass the event device as the first argument")
    return found[0]


path = (sys.argv[1] if len(sys.argv) > 1 else "") or default_device()
label = sys.argv[2] if len(sys.argv) > 2 else path
fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
samples = {c: [] for c in NAMES}
for _ in range(20):
    for code in NAMES:
        buf = bytearray(ABSINFO)
        try:
            fcntl.ioctl(fd, eviocgabs(code), buf)
        except OSError:
            continue  # the device has no such axis
        samples[code].append(struct.unpack("6i", buf)[0])
    time.sleep(0.05)
keys = bytearray(96)
fcntl.ioctl(fd, eviocgkey(len(keys)), keys)
held = [i for i in range(len(keys) * 8) if keys[i // 8] >> (i % 8) & 1]
print(f"== {label}")
for code, vals in samples.items():
    if vals:
        print(f"  {NAMES[code]:6} min={min(vals):4} max={max(vals):4}")
print("  held keys (evdev codes):", held)
