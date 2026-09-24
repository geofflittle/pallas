"""Apply one named break to leios.rs in a copy tree. Usage: mutate.py <copy dir> <name>."""
import sys

MUTANTS = {
    # The body hash comparison removed.
    "M1_body_hash_off": [("if found != announcement.eb_hash {", "if false && found != announcement.eb_hash {")],
    # The CertifiesNothing refusal removed: a parent without an announcement answers Ok(None).
    "M5_certifies_nothing_off": [(".ok_or(Error::CertifiesNothing {", ".or(Some(None)).ok_or(Error::CertifiesNothing {")],
    # The transaction hash taken over the envelope instead of its content.
    "M7_hash_envelope": [("let found = Hasher::<256>::hash(inner);", "let found = Hasher::<256>::hash(delivered.raw_bytes());")],
    # The transaction decoded from the envelope instead of its content.
    "M8_decode_envelope": [("decode_for_era(Era::Dijkstra, inner)", "decode_for_era(Era::Dijkstra, delivered.raw_bytes())")],
    # The count check weakened to refuse short deliveries only.
    "M11_long_delivery_ok": [("if wire.len() != self.len() {", "if wire.len() < self.len() {")],
    # The count refusal reports its two numbers swapped, every count still right.
    "M12_count_fields_swapped": [
        ("named: self.len(),", "named: wire.len(),"),
        ("delivered: wire.len(),", "delivered: self.len(),"),
    ],
    # A long delivery refused, but as a decode error rather than a count error.
    "M13_long_wrong_variant": [(
        "if wire.len() != self.len() {",
        "if wire.len() > self.len() { return Err(Error::TxDecode { index: self.len(), reason: String::new() }); }\n        if wire.len() < self.len() {",
    )],
    # The body hash taken over a re-encoding of the decoded map instead of the bytes that arrived.
    "C1_hash_reencoding": [("let found = block.original_hash();", "let found = Hasher::<256>::hash(&minicbor::to_vec(&*block).expect(\"encode\"));")],
    # The body hash refusal message stops naming the announced hash.
    "C2_message_drops_announced": [('#[error("endorser block hash {found}, announced {announced}")]', '#[error("endorser block hash {found}")]')],
}

root, name = sys.argv[1], sys.argv[2]
path = f"{root}/pallas-traverse/src/leios.rs"
src = open(path).read()
for old, new in MUTANTS[name]:
    n = src.count(old)
    assert n == 1, (name, old, n)
    src = src.replace(old, new)
open(path, "w").write(src)
print("applied", name)
