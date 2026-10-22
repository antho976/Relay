#!/usr/bin/env python3
"""A stand-in for adb in the engine's tests: one emulator, every command written to FAKE_ADB_LOG,
`screenrecord` "recording" until `pkill -INT screenrecord`, and `pull` handing back FAKE_ADB_CLIP."""
import os
import shutil
import sys
import time

log = os.environ["FAKE_ADB_LOG"]
args = sys.argv[1:]
with open(log, "a") as f:
    f.write(" ".join(args) + "\n")
stop = log + ".stop"
if args == ["devices"]:
    print("List of devices attached\nemulator-5554\tdevice\n")
    sys.exit(0)
if args[:2] == ["-s", "emulator-5554"]:
    args = args[2:]
if args[:2] == ["shell", "screenrecord"]:
    while not os.path.exists(stop):
        time.sleep(0.05)
    sys.exit(0)
if args[:3] == ["shell", "pkill", "-INT"]:
    open(stop, "w").close()
    sys.exit(0)
if args[:1] == ["pull"]:
    shutil.copy(os.environ["FAKE_ADB_CLIP"], args[2])
    sys.exit(0)
sys.exit(0)
