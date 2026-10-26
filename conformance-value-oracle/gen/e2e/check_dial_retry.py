"""Checks that a server call is repeated only when grpcurl could not dial the server.

usage: python3 -I e2e/check_dial_retry.py
"""

import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import served  # noqa: E402

FAKE = """#!/bin/sh
n=$(cat "{count}" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "{count}"
case "{mode}" in
  dial-once) [ "$n" -eq 1 ] && {{ echo 'Failed to dial target host "x": context deadline exceeded' >&2; exit 1; }} ;;
  dial-always) echo 'Failed to dial target host "x": context deadline exceeded' >&2; exit 1 ;;
  other) echo 'ERROR: Code: Internal' >&2; exit 1 ;;
esac
echo '{{"ok": 1}}'
"""


def run(mode, liar=False):
    """The result or the error of one call against a fake grpcurl, and how often the fake ran."""
    d = tempfile.mkdtemp()
    count = os.path.join(d, "count")
    fake = os.path.join(d, "grpcurl")
    with open(fake, "w") as fh:
        fh.write(FAKE.format(count=count, mode=mode))
    os.chmod(fake, 0o755)
    served.GRPCURL = fake
    if liar:
        os.environ["REAL_GRPCURL"] = fake
        served.GRPCURL = os.path.join(d, "liar")
        with open(served.GRPCURL, "w") as fh:
            fh.write(f'#!/bin/sh\nexec "{sys.executable}" -I "{os.path.join(HERE, "liar.py")}" "$@"\n')
        os.chmod(served.GRPCURL, 0o755)
    try:
        got = served.call("127.0.0.1:1", "m", {}, wait=0)
    except subprocess.CalledProcessError:
        got = "error"
    with open(count) as fh:
        return got, int(fh.read())


cases = [
    # Must not fire: a server error is not repeated.
    ("other error is not repeated", run("other") == ("error", 1)),
    # Must fire: a failed dial is repeated and the second answer is returned.
    ("failed dial is repeated", run("dial-once") == ({"ok": 1}, 2)),
    # Must fire: a server that never answers ends in the error after every attempt.
    ("dial attempts are bounded", run("dial-always") == ("error", served.DIAL_ATTEMPTS)),
    # Must not fire: a server error behind the liar is not repeated.
    ("other error behind the liar is not repeated", run("other", liar=True) == ("error", 1)),
    # Must fire: the liar passes a failed dial on, so it is repeated.
    ("failed dial behind the liar is repeated", run("dial-once", liar=True) == ({"ok": 1}, 2)),
]
bad = [name for name, ok in cases if not ok]
for name in bad:
    print(f"WRONG {name}")
print(f"dial retry: {len(cases)} cases, {len(bad)} wrong")
sys.exit(1 if bad else 0)
