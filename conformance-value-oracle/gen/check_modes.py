"""Checks the mutate.py modes that write an empty guards set and sub level keyhash guards, the given transaction's withdrawal order and the given block unwrapping.

usage: python3 -I check_modes.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from given import KEY_ACCOUNT, SCRIPT_ACCOUNT, bare, reward_order_tx  # noqa: E402
from mutate import GUARDS, bodies, copy, empty14, enc, subkeyhash  # noqa: E402
from value import Reader  # noqa: E402
from valuecmp import empty_guards  # noqa: E402


def term(hexstr):
    return copy(Reader(bytes.fromhex(hexstr)).item())


def guards(b):
    return next((v for k, v in b[1] if k == GUARDS), None)


CRED = ["list", [["int", 0], ["bytes", bytes([7] * 28)]]]
SUB = ["list", [["map", [[["int", 0], ["list", []]], [GUARDS, ["tag", 258, ["list", [CRED]]]]]], ["map", []], ["null"]]]
TX = ["list", [["map", [[["int", 0], ["list", []]], [["int", 23], ["list", [SUB]]]]], ["map", []], ["null"]]]
PLAIN = ["list", [["map", [[["int", 0], ["list", []]]]], ["map", []], ["null"]]]

checks = []
# Must not fire: a transaction with no key 14 anywhere is not an empty guards input.
checks.append(("no key 14 is not empty", not empty_guards("tx", enc(PLAIN).hex())))
# Must not fire: a nonempty sub level set is not an empty guards input.
checks.append(("nonempty is not empty", not empty_guards("tx", enc(TX).hex())))
# Must not fire: subkeyhash leaves a transaction without sub level guards unchanged.
checks.append(("subkeyhash without subs", not subkeyhash(PLAIN)[1]))
# Must fire: empty14 empties the top body and the sub body.
e, changed = empty14(TX)
top, subs = bodies(e)
checks.append(("empty14 top", changed and guards(top) == ["tag", 258, ["list", []]]))
checks.append(("empty14 sub", guards(subs[0]) == ["tag", 258, ["list", []]]))
checks.append(("empty14 is empty guards", empty_guards("tx", enc(e).hex())))
# Must fire: subkeyhash turns the sub credential into its hash.
k, changed = subkeyhash(TX)
checks.append(("subkeyhash sub", changed and guards(bodies(k)[1][0]) == ["tag", 258, ["list", [["bytes", bytes([7] * 28)]]]]))
# The given transaction writes the key account first, the order a raw byte sort keeps and the ledger reverses.
w = next(v for kk, v in bodies(term(reward_order_tx()))[0][1] if kk == ["int", 5])
checks.append(("given withdrawal order", [x[0][1] for x in w[1]] == [KEY_ACCOUNT, SCRIPT_ACCOUNT] and KEY_ACCOUNT < SCRIPT_ACCOUNT))

# A node block file is `[era, block]`, a bare block starts with its header list, and only blocks are unwrapped.
checks.append(("node block file", bare("block", "8208" + "8582a0a0") == "8582a0a0"))
checks.append(("bare block", bare("block", "8582a0a0") == "8582a0a0"))
checks.append(("tx unchanged", bare("tx", "8208a0") == "8208a0"))

bad = [name for name, ok in checks if not ok]
for name in bad:
    print(f"WRONG {name}")
print(f"mutation modes: {len(checks)} checks, {len(bad)} wrong")
sys.exit(1 if bad else 0)
