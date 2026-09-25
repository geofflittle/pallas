"""Count set arms (tag 258 or bare array) at body keys 0, 4, 13, 14, 18 of every transaction or sub transaction body in a block fixture."""
import sys
from collections.abc import Mapping

import cbor2


class Tagged(list):
    pass


def hook(decoder, tag):
    if isinstance(tag, cbor2.CBORTag) and tag.tag == 258:
        return Tagged(tag.value)
    return tag


def bodies(x, out):
    if isinstance(x, Mapping):
        if 0 in x and 1 in x and not isinstance(x[0], (bytes, int)):
            out.append(x)
        for v in x.values():
            bodies(v, out)
    elif isinstance(x, (list, tuple, set, frozenset)):
        for v in x:
            bodies(v, out)
    elif isinstance(x, cbor2.CBORTag):
        bodies(x.value, out)


for path in sys.argv[1:]:
    data = bytes.fromhex(open(path).read().strip())
    block = cbor2.loads(data, tag_hook=hook)
    found = []
    bodies(block, found)
    arms = {"tagged": 0, "bare": 0}
    for body in found:
        for key in (0, 4, 13, 14, 18):
            if key in body:
                tagged = isinstance(body[key], (Tagged, set, frozenset))
                arms["tagged" if tagged else "bare"] += 1
    print(path.rsplit("/", 1)[-1], "bodies", len(found), arms)
