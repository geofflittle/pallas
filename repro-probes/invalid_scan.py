"""Prints each era tagged block in the given hex files whose invalid transaction list is not empty."""
import importlib.util
import pathlib
import sys

spec = importlib.util.spec_from_file_location("o", str(pathlib.Path(__file__).with_name("subtx_oracle.py")))
o = importlib.util.module_from_spec(spec)
spec.loader.exec_module(o)

for path in sys.argv[1:]:
    try:
        b = bytes.fromhex(pathlib.Path(path).read_text().strip())
        top = o.items(b, 0)
        era = o.uint(b, top[0])
        block = o.items(b, top[1])
        if era < 4 or len(block) != 5:
            continue
        invalid = o.items(b, block[4])
        if invalid:
            print(path, "era", era, "txs", len(o.items(b, block[1])), "invalid", [o.uint(b, x) for x in invalid])
    except Exception as e:  # noqa: BLE001
        print(path, "skipped", type(e).__name__, e, file=sys.stderr)
