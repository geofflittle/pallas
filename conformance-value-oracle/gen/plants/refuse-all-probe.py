#!/usr/bin/env python3
"""A probe that refuses every input, the refuse-all plant.

usage: refuse-all-probe.py KIND < GEN > PROBE
"""

import json
import sys

for line in sys.stdin:
    if line.strip():
        print(json.dumps({"ok": False, "err": "planted refusal"}))
