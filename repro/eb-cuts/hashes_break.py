"""Drop dijkstra16 from both fixture lists of hashes.rs in a copy tree. Usage: hashes_break.py <copy dir>."""
import sys

path = f"{sys.argv[1]}/pallas-traverse/src/hashes.rs"
src = open(path).read()
case = """            (
                include_str!("../../test_data/dijkstra16.block"),
                "c9d7bca094227279830e2e2110acbb965dc9e90d469ac97594d40bc8e295735c",
                14935,
                311025,
                crate::Era::Dijkstra,
            ),
"""
listed = """            include_str!("../../test_data/dijkstra16.block"),
        ];
"""
for old, new in [(case, ""), (listed, "        ];\n")]:
    assert src.count(old) == 1, old
    src = src.replace(old, new)
open(path, "w").write(src)
print("applied hashes_drop_dijkstra16")
