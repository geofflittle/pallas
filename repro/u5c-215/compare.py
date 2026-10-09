# Compare two probe output dirs and summarise which JSON paths differ, by file.
import json, os, sys, collections, threading
sys.setrecursionlimit(1000000)
threading.stack_size(1 << 29)
a_dir, b_dir = sys.argv[1], sys.argv[2]
def walk(a, b, path, out):
    if type(a) != type(b):
        out.append((path, a, b)); return
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            walk(a.get(k), b.get(k), path + "." + k, out)
    elif isinstance(a, list):
        if len(a) != len(b):
            out.append((path + "[len]", len(a), len(b)))
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, path + "[]", out)
    elif a != b:
        out.append((path, a, b))
def main():
    files = sorted(os.listdir(a_dir))
    changed = 0
    kinds = collections.Counter()
    for f in files:
        ta = open(os.path.join(a_dir, f)).read(); tb = open(os.path.join(b_dir, f)).read()
        try:
            A = json.loads(ta); B = json.loads(tb)
        except RecursionError:
            print("DEEP", f, "text_equal" if ta == tb else "TEXT_DIFFERS")
            if ta != tb: changed += 1
            continue
        diffs = []
        walk(A, B, "", diffs)
        if diffs:
            changed += 1
            ps = sorted(set(p for p, _, _ in diffs))
            for p in ps: kinds[p] += 1
            print("CHANGED", f, len(diffs), "e.g.", diffs[0])
    print("TOTAL files", len(files), "changed", changed)
    for k, v in kinds.most_common(): print("PATH", v, k)
t = threading.Thread(target=main)
t.start()
t.join()
