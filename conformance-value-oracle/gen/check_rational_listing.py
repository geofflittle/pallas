"""Checks that a rational too wide for 32 bits is listed only as its own leaf and only when the schema holds the nearest value that fits.

usage: python3 -I check_rational_listing.py
"""

import os
import sys
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schemacmp import ABSENT, listed, nearest  # noqa: E402

MAX_NUM, MAX_DEN = 2**31 - 1, 2**32 - 1


def r(f):
    return f"r:{f.numerator}/{f.denominator}"


# The nearest values, each read by the standard library alone: by denominator where only the denominator binds,
# and through the reciprocal where only the numerator binds.
WIDE_DEN = Fraction(1, 2**32 + 1)
NEAR_WIDE_DEN = WIDE_DEN.limit_denominator(MAX_DEN)
WIDE_NUM = Fraction(5_000_000_000, 7)
NEAR_WIDE_NUM = 1 / (1 / WIDE_NUM).limit_denominator(MAX_NUM)
assert NEAR_WIDE_DEN == Fraction(1, 2**32 - 1) and NEAR_WIDE_NUM.denominator <= MAX_DEN

cases = [
    # Must be listed: the leaf is too wide and the schema holds the nearest value that fits.
    ("wide denominator, nearest", "margin", r(WIDE_DEN), r(NEAR_WIDE_DEN), True),
    ("wide numerator, nearest", "margin", r(WIDE_NUM), r(NEAR_WIDE_NUM), True),
    # Must not be listed: the leaf is too wide and the schema holds another value.
    ("wide denominator, wrapped", "margin", r(WIDE_DEN), "r:1/1", False),
    ("wide numerator, wrapped", "margin", r(WIDE_NUM), r(Fraction(5_000_000_000 % 2**32, 7)), False),
    ("wide denominator, absent", "margin", r(WIDE_DEN), ABSENT, False),
    # Must not be listed: a value that fits, or a sibling of the wide leaf.
    ("fits", "margin", "r:1/3", "r:1/2", False),
    ("sibling", "certs[]", {"margin": r(WIDE_DEN), "pledge": "5"}, {"margin": r(NEAR_WIDE_DEN), "pledge": "6"}, False),
]
bad = [name for name, gp, lv, sv, want in cases if (listed("v1", gp, lv, sv) is not None) != want]


def brute(n, d, max_num, max_den):
    x = Fraction(n, d)
    return min(((p, q) for q in range(1, max_den + 1) for p in range(max_num + 1)), key=lambda c: (abs(Fraction(*c) - x), c[1], c[0]))


# Every n/d in a small box against an exhaustive search under the same bounds.
small = [(n, d, mn, md) for mn, md in ((7, 11), (11, 7), (1, 1), (5, 5)) for d in range(1, 25) for n in range(0, 90)]
wrong = [s for s in small if Fraction(*nearest(*s)) != Fraction(*brute(*s)) or nearest(*s)[1] != brute(*s)[1]]
bad += [f"nearest {s}: {nearest(*s)} where {brute(*s)}" for s in wrong[:5]]
print(f"nearest against exhaustive search: {len(small)} cases, {len(wrong)} wrong")
for name in bad:
    print(f"WRONG {name}")
print(f"wide rational listing: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
