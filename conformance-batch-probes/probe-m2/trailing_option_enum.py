"""Lists variants of derived Encode enums whose last field is an Option."""
import re, sys, pathlib
for p in sorted(pathlib.Path(sys.argv[1]).rglob("*.rs")):
    if "/target/" in str(p):
        continue
    src = p.read_text()
    for m in re.finditer(r"#\[derive\([^)]*\bEncode\b[^)]*\)\]\s*((?:#\[[^\]]*\]\s*)*)pub enum (\w+)[^{]*\{", src):
        attrs, name = m.group(1), m.group(2)
        i = m.end(); depth = 1; start = i
        while depth:
            c = src[i]
            depth += c in "{([" and 1 or 0
            depth -= c in "})]" and 1 or 0
            i += 1
        body = src[start:i - 1]
        # split top level variants
        parts, d, cur = [], 0, ""
        for c in body:
            if c in "({[": d += 1
            if c in ")}]": d -= 1
            if c == "," and d == 0:
                parts.append(cur); cur = ""
            else:
                cur += c
        parts.append(cur)
        for v in parts:
            v = re.sub(r"//[^\n]*", "", v)
            vm = re.search(r"(\w+)\s*[({](.*)[)}]\s*$", v.strip(), re.S)
            if not vm:
                continue
            fields = [f.strip() for f in re.split(r",(?![^<]*>)", vm.group(2)) if f.strip()]
            if fields and re.sub(r"#\[[^\]]*\]\s*", "", fields[-1]).split(":")[-1].strip().startswith("Option<"):
                line = src[:m.start()].count("\n") + 1
                print(f"{p}:{line} {name}::{vm.group(1)} flat={'flat' in attrs} last={fields[-1]}")
