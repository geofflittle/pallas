"""Checks that every container the harness scripts start keeps no log on the host disk.

usage: python3 -I check_docker_logs.py [DIR]
"""

import pathlib
import sys

root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).parent)
lines = 0
wrong = 0
for path in sorted(root.rglob("*")):
    if path.suffix not in (".sh", ".py") or path.name == pathlib.Path(__file__).name:
        continue
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if "docker run" not in line:
            continue
        lines += 1
        if "--log-driver none" not in line:
            wrong += 1
            print(f"WRONG {path.relative_to(root)}:{number} {line.strip()}")
print(f"docker run lines: {lines}, without --log-driver none: {wrong}")
sys.exit(1 if wrong or not lines else 0)
