"""Lists derived Encode array structs whose last field is an Option."""
import re, sys, pathlib
for p in sorted(pathlib.Path(sys.argv[1]).rglob("*.rs")):
    lines = p.read_text().splitlines()
    i = 0
    while i < len(lines):
        if re.match(r"\s*#\[derive\(.*\bEncode\b", lines[i]):
            attrs = []
            j = i
            while j < len(lines) and not re.match(r"\s*pub (struct|enum) ", lines[j]):
                attrs.append(lines[j]); j += 1
            if j < len(lines) and "struct" in lines[j] and lines[j].rstrip().endswith("{"):
                name = lines[j].split("struct")[1].split("{")[0].strip()
                k = j + 1; fields = []
                while k < len(lines) and lines[k].strip() != "}":
                    m = re.match(r"\s*pub (\w+): (.*),\s*$", lines[k])
                    if m: fields.append((m.group(1), m.group(2), k + 1))
                    k += 1
                ismap = any("cbor(map" in a for a in attrs)
                if fields and not ismap and fields[-1][1].startswith("Option<"):
                    f = fields[-1]
                    print(f"{p}:{f[2]} {name}.{f[0]}: {f[1]}")
            i = j
        i += 1
