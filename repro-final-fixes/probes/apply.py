import sys, pathlib
tree, which, probes = sys.argv[1], sys.argv[2].split(","), pathlib.Path(sys.argv[3])
root = pathlib.Path(tree)
if "shelley" in which:
    p = root / "pallas-validate/src/phase1/shelley_ma.rs"
    p.write_text(p.read_text() + (probes / "shelley_probe.rs").read_text())
if "utils" in which:
    p = root / "pallas-validate/src/utils.rs"
    p.write_text(p.read_text() + (probes / "utils_probe.rs").read_text())
if "conway" in which:
    p = root / "pallas-validate/tests/conway.rs"
    text = p.read_text().rstrip()
    assert text.endswith("}")
    p.write_text(text[:-1] + (probes / "conway_phase2_probe.rs").read_text())
for era in ("babbage", "alonzo"):
    if era + "_balance" in which:
        p = root / ("pallas-validate/src/phase1/%s.rs" % era)
        p.write_text(p.read_text() + (probes / ("%s_balance_probe.rs" % era)).read_text())
print("applied", which, "to", tree)
