"""The expected differences between pallas and the ledger, each a listed class the comparers may count instead of a disagreement.

A run fails when any entry here matches no value, and when a comparer lists a value under a class not here.
"""

KEEPS_INPUT = "ledger re-encodes, pallas keeps the input bytes"
OTHER_BYTES = "pallas re-encodes to other bytes of the same ledger value"
EMPTY_GUARDS_NEXT_NODE = (
    "the next node refuses an empty guards set, pallas and the pinned ledger read it as no guards, "
    "and the refusal rule passes emptiness through"
)
EB_LIST_FORM = "the pinned ledger writes eb_references as a list of pairs, pallas reads the node's map"

RATIONAL = "type cannot hold: RationalNumber is int32 over uint32, so it holds the nearest value"
METADATUM = "type cannot hold: Metadatum int is int64"
PERAS_EMPTY = "type cannot hold: empty Peras certificate bytes read as no certificate"
DNS_EMPTY = "type cannot hold: an empty dns name reads as no name"
NOFK = "type cannot hold: ScriptNOfK k is uint32"
EXPIRES = "type cannot hold: NewCommitteeCredentials expires_epoch is uint32"
PORT_ZERO = "type cannot hold: a uint32 port of 0 reads as no port"
NO_GUARD_SCRIPT = "type cannot hold: no guard native script variant"
NO_GUARDING_PURPOSE = "type cannot hold: no guarding redeemer purpose"
NO_TOTAL_COLLATERAL = "legacy mapping: no total collateral maps to 0"
NO_AUX = "legacy mapping: no auxiliary data maps to an empty one"

VALUE = (KEEPS_INPUT, OTHER_BYTES, EMPTY_GUARDS_NEXT_NODE, EB_LIST_FORM)
SCHEMA = (
    RATIONAL,
    METADATUM,
    PERAS_EMPTY,
    DNS_EMPTY,
    NOFK,
    EXPIRES,
    PORT_ZERO,
    NO_GUARD_SCRIPT,
    NO_GUARDING_PURPOSE,
    NO_TOTAL_COLLATERAL,
    NO_AUX,
)
