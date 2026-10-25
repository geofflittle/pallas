"""Checks that the open tag 24 refusal class lists only an indefinite byte string directly under tag 24.

usage: python3 -I check_listing.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from valuecmp import tag24_indefinite  # noqa: E402

ERR = "unexpected type indefinite bytes at position {}: expected bytes (definite length)"

cases = [
    # Must not fire: indefinite bytes refused at a position without a tag 24 head before it.
    (ERR.format(3), "8201025f4101ff", False),
    # Must not fire: tag 24 present but the refusal is another error.
    ("unexpected type map at position 2: expected bytes (definite length)", "d818a0", False),
    # Must not fire: tag 24 head elsewhere in the input.
    (ERR.format(5), "d818410082015f4101ff", False),
    # Must fire: the refusal points at the 5f byte right after d8 18.
    (ERR.format(2), "d8185f4101ff", True),
    (ERR.format(1899), "00" * 1897 + "d8185f4101ff", True),
]
bad = [(e, h, want) for e, h, want in cases if tag24_indefinite(e, h) != want]
for e, h, want in bad:
    print(f"WRONG want {want}: {e} {h[:40]}")
print(f"tag 24 listing: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
