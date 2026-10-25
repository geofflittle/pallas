"""Checks that value.rows yields aligned rows from files of equal length and exits 2 when one file is shorter or all are empty.

usage: python3 -I check_rows.py
"""

import contextlib
import io
import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from value import rows  # noqa: E402


def write(d, name, lines):
    path = os.path.join(d, name)
    with open(path, "w") as fh:
        fh.write("".join(lines))
    return path


def run(paths):
    out = io.StringIO()
    got, code = [], None
    with contextlib.redirect_stdout(out):
        try:
            got = list(rows(*paths))
        except SystemExit as e:
            code = e.code
    return got, code, out.getvalue()


def main():
    wrong = 0
    cases = 0
    with tempfile.TemporaryDirectory() as d:
        a = write(d, "a", ['{"n": 0}\n', "\n", '{"n": 1}\n'])
        b = write(d, "b", ['{"m": 0}\n', '{"m": 1}\n'])
        short = write(d, "short", ['{"m": 0}\n'])
        # Must not fire: equal counts, a blank line skipped.
        cases += 1
        got, code, _ = run([a, b])
        if code is not None or got != [({"n": 0}, {"m": 0}), ({"n": 1}, {"m": 1})]:
            wrong += 1
            print(f"equal files: got {got} exit {code}")
        # Must fire: the second file ends one row early.
        cases += 1
        got, code, msg = run([a, short])
        if code != 2 or "row 1 is missing from" not in msg or short not in msg or a in msg.split("from", 1)[1]:
            wrong += 1
            print(f"short second file: exit {code} message {msg!r}")
        # Must fire: the first file ends one row early.
        cases += 1
        got, code, msg = run([short, b])
        if code != 2 or short not in msg:
            wrong += 1
            print(f"short first file: exit {code} message {msg!r}")
        # Must fire: every file is empty, so there is nothing to compare.
        empty = write(d, "empty", [])
        blank = write(d, "blank", ["\n"])
        cases += 1
        got, code, msg = run([empty, blank])
        if code != 2 or "no rows" not in msg:
            wrong += 1
            print(f"empty files: exit {code} message {msg!r}")
    print(f"aligned rows: {cases} cases, {wrong} wrong")
    sys.exit(1 if wrong else 0)


if __name__ == "__main__":
    main()
