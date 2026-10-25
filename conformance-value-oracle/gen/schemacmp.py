"""Compares each utxorpc schema message with the ledger value, through one path table per schema.

usage: python3 -I schemacmp.py KIND GEN ORACLE PROBE OUT_PREFIX
       python3 -I schemacmp.py --tables

The probe prints, for each input, the serde JSON of the v1alpha, v1beta and v1
messages pallas maps it to. Each message is turned back into the shape of the
ledger dump, field by field, and compared with the ledger dump under the rows
of that schema's path table. A ledger path no row covers is listed once per
schema as "no field in SCHEMA". A covered path where the two differ is a
disagreement, and so is a mapper that refuses an input the ledger accepts.
Exits 1 when any disagreement is found.
"""

import base64
import json
import os
import re
import sys
from collections import Counter
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from value import canon, flatten, generic, key_of, normalize, rows, segment  # noqa: E402

SCHEMAS = ("v1alpha", "v1beta", "v1")

# Values in the normalized shape `value.normalize` gives.


def h(b64):
    return "h:" + base64.b64decode(b64 or "").hex()


def hexs(b64):
    return base64.b64decode(b64 or "").hex()


def num(x):
    return str(int(x or 0))


def big(b):
    if b is None:
        return None
    if "int" in b:
        return str(int(b["int"]))
    if "bigUInt" in b:
        return str(int.from_bytes(base64.b64decode(b["bigUInt"]), "big"))
    if "bigNInt" in b:
        return str(-1 - int.from_bytes(base64.b64decode(b["bigNInt"]), "big"))
    return "0"


def rat(r):
    if r is None:
        return None
    n, d = int(r.get("numerator", 0)), int(r.get("denominator", 0))
    if d == 0:
        return {"tag": "30", "v": [str(n), "0"]}
    f = Fraction(n, d)
    return f"r:{f.numerator}/{f.denominator}"


def nset(items):
    """A set keyed by value, with each repeat of a member kept under the member's key and its repeat number."""
    out, seen = {}, Counter()
    for c, v in sorted((canon(x), x) for x in items):
        seen[c] += 1
        out[key_of(v) if seen[c] == 1 else f"{key_of(v)}#{seen[c]}"] = v
    return {"set": out}


def noset(items):
    return {"oset": list(items)}


def nmap(pairs):
    out = {}
    for k, v in pairs:
        out[canon(k)] = (k, v)
    return {"map": {key_of(k): {"k": k, "v": v} for k, v in out.values()}}


def nomap(pairs):
    out = {}
    for k, v in pairs:
        out.setdefault(canon(k), (k, v))
    entries = {key_of(k): {"k": k, "v": v} for k, v in out.values()}
    return {"omap": entries, "order": [key_of(k) for k, _ in out.values()]}


def cmap(pairs):
    return {"m": {canon(k): v for k, v in pairs}}


def opt(x, f):
    return None if x is None else f(x)


# Schema messages in the ledger dump shape.


def cred(c):
    if c is None:
        return None
    if "addrKeyHash" in c:
        return ["0", h(c["addrKeyHash"])]
    if "scriptHash" in c:
        return ["1", h(c["scriptHash"])]
    return {"empty credential": True}


def data(d):
    if d is None:
        return None
    if "constr" in d:
        c = d["constr"]
        tag = int(c.get("tag", 0))
        if 121 <= tag <= 127:
            ix = tag - 121
        elif 1280 <= tag <= 1400:
            ix = tag - 1280 + 7
        else:
            ix = int(c.get("anyConstructor", 0))
        return {"tag": "constr", "ix": str(ix), "fields": [data(x) for x in c.get("fields", [])]}
    if "map" in d:
        return {"tag": "map", "entries": [[data(p.get("key")), data(p.get("value"))] for p in d["map"].get("pairs", [])]}
    if "array" in d:
        return {"tag": "list", "items": [data(x) for x in d["array"].get("items", [])]}
    if "bigInt" in d:
        return {"tag": "int", "v": big(d["bigInt"])}
    if "boundedBytes" in d:
        return {"tag": "bytes", "v": hexs(d["boundedBytes"])}
    return {"tag": "bytes", "v": ""} if d == {} else {"unknown data": sorted(d)}


def native(n):
    if n is None:
        return None
    if "scriptPubkeyHash" in n or "scriptPubkey" in n:
        return {"tag": "sig", "hash": h(n.get("scriptPubkeyHash", n.get("scriptPubkey")))}
    if "scriptAll" in n:
        return {"tag": "all", "scripts": [native(x) for x in n["scriptAll"].get("items", [])]}
    if "scriptAny" in n:
        return {"tag": "any", "scripts": [native(x) for x in n["scriptAny"].get("items", [])]}
    if "scriptNOfK" in n:
        k = n["scriptNOfK"]
        return {"tag": "m_of", "m": num(k.get("k")), "scripts": [native(x) for x in k.get("scripts", [])]}
    if "invalidBefore" in n:
        return {"tag": "start", "slot": num(n["invalidBefore"])}
    if "invalidHereafter" in n:
        return {"tag": "expire", "slot": num(n["invalidHereafter"])}
    if "scriptRequireGuard" in n:
        return {"tag": "guard", "cred": cred(n["scriptRequireGuard"])}
    return {"tag": "none"}


PLUTUS = {"plutusV1": "plutus_v1", "plutusV2": "plutus_v2", "plutusV3": "plutus_v3", "plutusV4": "plutus_v4"}


def script(s):
    if s is None:
        return None
    if "native" in s:
        return {"tag": "native", "script": native(s["native"])}
    for k, name in PLUTUS.items():
        if k in s:
            return {"tag": name, "bytes": hexs(s[k])}
    return {"tag": "none"}


def multi(ms):
    return nmap(
        [(h(m.get("policyId")), nmap([(h(a.get("name")), big(quantity(a))) for a in m.get("assets", [])])) for m in ms]
    )


def quantity(a):
    """The asset quantity, which v1alpha holds in one of two members and v1beta and v1 in one."""
    for k in ("quantity", "outputCoin", "mintCoin"):
        if k in a:
            return a[k]
    return None


def datum(d):
    if d is None:
        return None
    if d.get("payload") is not None:
        return {"tag": "inline", "data": data(d["payload"])}
    if d.get("hash"):
        return {"tag": "hash", "hash": h(d["hash"])}
    return None


def output(o):
    if o is None:
        return None
    return {
        "address": h(o.get("address")),
        "coin": big(o.get("coin")) or "0",
        "assets": multi(o.get("assets", [])),
        "datum": datum(o.get("datum")),
        "script": script(o.get("script")),
    }


def txin(i):
    return [h(i.get("txHash")), num(i.get("outputIndex"))]


def anchor(a):
    return None if a is None else ["t:" + a.get("url", ""), h(a.get("contentHash"))]


def drep(d):
    if d is None:
        return None
    if "addrKeyHash" in d:
        return ["0", h(d["addrKeyHash"])]
    if "scriptHash" in d:
        return ["1", h(d["scriptHash"])]
    if "abstain" in d:
        return ["2"]
    if "noConfidence" in d:
        return ["3"]
    return {"empty drep": True}


def relay(r):
    """The parts of a relay the schema carries: addresses, name and port, without the relay kind."""
    out = {}
    if r.get("ipV4"):
        out["ip4"] = h(r["ipV4"])
    if r.get("ipV6"):
        out["ip6"] = h(r["ipV6"])
    if r.get("dnsName"):
        out["dns"] = "t:" + r["dnsName"]
    if r.get("port"):
        out["port"] = num(r["port"])
    return out


def cert(c):
    if "poolRegistration" in c:
        p = c["poolRegistration"]
        bls = p.get("blsKey")
        md = p.get("poolMetadata")
        return {
            "tag": "pool_reg",
            "operator": h(p.get("operator")),
            "vrf": h(p.get("vrfKeyhash")),
            "bls": None if bls is None else [h(bls.get("blsPubkey")), h(bls.get("blsPossessionProof"))],
            "pledge": big(p.get("pledge")) or "0",
            "cost": big(p.get("cost")) or "0",
            "margin": rat(p.get("margin")),
            "account": h(p.get("rewardAccount")),
            "owners": nset([h(x) for x in p.get("poolOwners", [])]),
            "relays": [relay(r) for r in p.get("relays", [])],
            "metadata": None if md is None else ["t:" + md.get("url", ""), h(md.get("hash"))],
        }
    simple = {
        "stakeRegistration": lambda x: ["0", cred(x)],
        "stakeDeregistration": lambda x: ["1", cred(x)],
        "stakeDelegation": lambda x: ["2", cred(x.get("stakeCredential")), h(x.get("poolKeyhash"))],
        "poolRetirement": lambda x: ["4", h(x.get("poolKeyhash")), num(x.get("epoch"))],
        "regCert": lambda x: ["7", cred(x.get("stakeCredential")), big(x.get("coin")) or "0"],
        "unregCert": lambda x: ["8", cred(x.get("stakeCredential")), big(x.get("coin")) or "0"],
        "voteDelegCert": lambda x: ["9", cred(x.get("stakeCredential")), drep(x.get("drep"))],
        "stakeVoteDelegCert": lambda x: ["10", cred(x.get("stakeCredential")), h(x.get("poolKeyhash")), drep(x.get("drep"))],
        "stakeRegDelegCert": lambda x: ["11", cred(x.get("stakeCredential")), h(x.get("poolKeyhash")), big(x.get("coin")) or "0"],
        "voteRegDelegCert": lambda x: ["12", cred(x.get("stakeCredential")), drep(x.get("drep")), big(x.get("coin")) or "0"],
        "stakeVoteRegDelegCert": lambda x: [
            "13",
            cred(x.get("stakeCredential")),
            h(x.get("poolKeyhash")),
            drep(x.get("drep")),
            big(x.get("coin")) or "0",
        ],
        "authCommitteeHotCert": lambda x: ["14", cred(x.get("committeeColdCredential")), cred(x.get("committeeHotCredential"))],
        "resignCommitteeColdCert": lambda x: ["15", cred(x.get("committeeColdCredential")), anchor(x.get("anchor"))],
        "regDrepCert": lambda x: ["16", cred(x.get("drepCredential")), big(x.get("coin")) or "0", anchor(x.get("anchor"))],
        "unregDrepCert": lambda x: ["17", cred(x.get("drepCredential")), big(x.get("coin")) or "0"],
        "updateDrepCert": lambda x: ["18", cred(x.get("drepCredential")), anchor(x.get("anchor"))],
    }
    for k, f in simple.items():
        if k in c:
            return f(c[k])
    return {"unmapped certificate": sorted(x for x in c if x != "redeemer")}


def exunits(u):
    return None if u is None else [num(u.get("memory")), num(u.get("steps"))]


def costmodels(cm):
    keys = {"plutusV1": "0", "plutusV2": "1", "plutusV3": "2", "plutusV4": "3"}
    return cmap([(keys[k], [num(v) for v in m.get("values", [])]) for k, m in cm.items() if k in keys])


def thresholds(t):
    return [rat(r) for r in t.get("thresholds", [])]


def leverage(x):
    return rat(x["leverage"]) if "leverage" in x else None


def bootstrap(x):
    return num(x["round"]) if "round" in x else None


# One row per ledger key: (key, schema field, conversion, schemas that carry it).
PPU = [
    (0, "minFeeCoefficient", big, SCHEMAS),
    (1, "minFeeConstant", big, SCHEMAS),
    (2, "maxBlockBodySize", num, SCHEMAS),
    (3, "maxTxSize", num, SCHEMAS),
    (4, "maxBlockHeaderSize", num, SCHEMAS),
    (5, "stakeKeyDeposit", big, SCHEMAS),
    (6, "poolDeposit", big, SCHEMAS),
    (7, "poolRetirementEpochBound", num, SCHEMAS),
    (8, "desiredNumberOfPools", num, SCHEMAS),
    (9, "poolInfluence", rat, SCHEMAS),
    (10, "monetaryExpansion", rat, SCHEMAS),
    (11, "treasuryExpansion", rat, SCHEMAS),
    (16, "minPoolCost", big, SCHEMAS),
    (17, "coinsPerUtxoByte", big, SCHEMAS),
    (18, "costModels", costmodels, SCHEMAS),
    (19, "prices", lambda p: [rat(p.get("memory")), rat(p.get("steps"))], SCHEMAS),
    (20, "maxExecutionUnitsPerTransaction", exunits, SCHEMAS),
    (21, "maxExecutionUnitsPerBlock", exunits, SCHEMAS),
    (22, "maxValueSize", num, SCHEMAS),
    (23, "collateralPercentage", num, SCHEMAS),
    (24, "maxCollateralInputs", num, SCHEMAS),
    (25, "poolVotingThresholds", thresholds, SCHEMAS),
    (26, "drepVotingThresholds", thresholds, SCHEMAS),
    (27, "minCommitteeSize", num, SCHEMAS),
    (28, "committeeTermLimit", num, SCHEMAS),
    (29, "governanceActionValidityPeriod", num, SCHEMAS),
    (30, "governanceActionDeposit", big, SCHEMAS),
    (31, "drepDeposit", big, SCHEMAS),
    (32, "drepInactivityPeriod", num, SCHEMAS),
    (33, "minFeeScriptRefCostPerByte", rat, SCHEMAS),
    (34, "maxRefScriptSizePerBlock", num, ("v1",)),
    (35, "maxRefScriptSizePerTx", num, ("v1",)),
    (36, "refScriptCostStride", num, ("v1",)),
    (37, "refScriptCostMultiplier", rat, ("v1",)),
    (38, "maxPledgeLeverage", leverage, ("v1",)),
    (39, "minPoolMargin", rat, ("v1",)),
    (40, "leiosAnnouncementPeriodLength", num, ("v1",)),
    (41, "leiosVotePeriodLength", num, ("v1",)),
    (42, "leiosDiffusionPeriodLength", num, ("v1",)),
    (43, "leiosCommitteeSize", num, ("v1",)),
    (44, "leiosQuorumStakeThreshold", rat, ("v1",)),
    (45, "maxEndorserBlockReferencesSize", num, ("v1",)),
    (46, "maxEndorserBlockTxsSize", num, ("v1",)),
    (47, "maxEndorserBlockExecutionUnits", exunits, ("v1",)),
    (48, "maxRefScriptSizePerEndorserBlock", num, ("v1",)),
    (49, "perasMinCandidateBlockAge", num, ("v1",)),
    (50, "perasHealingFactor", rat, ("v1",)),
    (51, "perasCertBoost", num, ("v1",)),
    (52, "perasTargetCommitteeSize", num, ("v1",)),
    (53, "perasBootstrapRound", bootstrap, ("v1",)),
    (54, "perasQuorumThresholdSafetyMargin", rat, ("v1",)),
    (55, "refInputsCostPerMultiAssetPolicy", big, ("v1",)),
    (56, "refInputsCostPerDatumByte", big, ("v1",)),
]


def ppu(p):
    return cmap([(str(k), f(p[field])) for k, field, f, _ in PPU if field in p])


def gaid(g):
    return None if g is None else [h(g.get("transactionId")), num(g.get("governanceActionIndex"))]


def action(g):
    if g is None:
        return None
    if "parameterChangeAction" in g:
        x = g["parameterChangeAction"]
        return ["0", gaid(x.get("govActionId")), ppu(x.get("protocolParamUpdate", {})), opt(x.get("policyHash"), h)]
    if "hardForkInitiationAction" in g:
        x = g["hardForkInitiationAction"]
        v = x.get("protocolVersion", {})
        return ["1", gaid(x.get("govActionId")), [num(v.get("major")), num(v.get("minor"))]]
    if "treasuryWithdrawalsAction" in g:
        x = g["treasuryWithdrawalsAction"]
        return {
            "tag": "treasury_withdrawals",
            "withdrawals": nmap([(h(w.get("rewardAccount")), big(w.get("coin")) or "0") for w in x.get("withdrawals", [])]),
            "policy": opt(x.get("policyHash"), h),
        }
    if "noConfidenceAction" in g:
        return ["3", gaid(g["noConfidenceAction"].get("govActionId"))]
    if "updateCommitteeAction" in g:
        x = g["updateCommitteeAction"]
        return {
            "tag": "update_committee",
            "prev": gaid(x.get("govActionId")),
            "remove": nset([cred(c) for c in x.get("removeCommitteeCredentials", [])]),
            "add": nmap(
                [(cred(c.get("committeeColdCredential")), num(c.get("expiresEpoch"))) for c in x.get("newCommitteeCredentials", [])]
            ),
            "threshold": rat(x.get("newCommitteeThreshold")),
        }
    if "newConstitutionAction" in g:
        x = g["newConstitutionAction"]
        con = x.get("constitution", {})
        return ["5", gaid(x.get("govActionId")), [anchor(con.get("anchor")), opt(con.get("hash"), h)]]
    if "infoAction" in g:
        return ["6"]
    return {"unmapped action": sorted(g)}


def proposal(p):
    return {
        "deposit": big(p.get("deposit")) or "0",
        "account": h(p.get("rewardAccount")),
        "action": action(p.get("govAction")),
        "anchor": anchor(p.get("anchor")),
    }


VOTE = {"VOTE_NO": "0", "VOTE_YES": "1", "VOTE_ABSTAIN": "2"}


def voter(v):
    if "constitutionalCommittee" in v:
        c = cred(v["constitutionalCommittee"])
        return ["0" if c[0] == "0" else "1", c[1]]
    if "drep" in v:
        c = cred(v["drep"])
        return ["2" if c[0] == "0" else "3", c[1]]
    if "spo" in v:
        return ["4", h(v["spo"])]
    return {"empty voter": True}


def votes(vs):
    return nmap(
        [
            (
                voter(v),
                nmap([(gaid(p.get("govActionId")), [VOTE.get(p.get("vote"), "?"), anchor(p.get("anchor"))]) for p in v.get("votes", [])]),
            )
            for v in vs
        ]
    )


def interval(i):
    if "exact" in i:
        return big(i["exact"])
    r = i.get("range", {})
    return [big(r.get("inclusiveLowerBound")), big(r.get("exclusiveUpperBound"))]


def intervals(xs):
    return nmap([(h(x.get("rewardAccount")), interval(x)) for x in xs])


PURPOSE = {
    "REDEEMER_PURPOSE_SPEND": "0",
    "REDEEMER_PURPOSE_MINT": "1",
    "REDEEMER_PURPOSE_CERT": "2",
    "REDEEMER_PURPOSE_REWARD": "3",
    "REDEEMER_PURPOSE_VOTE": "4",
    "REDEEMER_PURPOSE_PROPOSE": "5",
    "REDEEMER_PURPOSE_GUARDING": "6",
}


def wits(w):
    w = w or {}
    return {
        "vkeys": nset([[h(x.get("vkey")), h(x.get("signature"))] for x in w.get("vkeywitness", [])]),
        "boots": nset(
            [[h(x.get("vkey")), h(x.get("signature")), h(x.get("chainCode")), h(x.get("attributes"))] for x in w.get("bootstrapWitnesses", [])]
        ),
        "scripts": nset([script(x) for x in w.get("script", [])]),
        "datums": nset([data(x) for x in w.get("plutusDatums", [])]),
        "redeemers": nmap(
            [
                ([PURPOSE.get(r.get("purpose"), "?"), num(r.get("index"))], {"data": data(r.get("payload")), "units": exunits(r.get("exUnits"))})
                for r in w.get("redeemers", [])
            ]
        ),
    }


def metadatum(m):
    if m is None:
        return None
    if "int" in m:
        return num(m["int"])
    if "bytes" in m:
        return h(m["bytes"])
    if "text" in m:
        return "t:" + m["text"]
    if "array" in m:
        return [metadatum(x) for x in m["array"].get("items", [])]
    if "map" in m:
        return cmap([(metadatum(p.get("key")), metadatum(p.get("value"))) for p in m["map"].get("pairs", [])])
    return {"empty metadatum": True}


def aux(a):
    if a is None:
        return None
    scripts = a.get("scripts", [])
    plutus = {}
    for s in scripts:
        for k, name in PLUTUS.items():
            if k in s:
                plutus.setdefault(name, []).append(hexs(s[k]))
    return {
        "metadata": nmap([(num(m.get("label")), metadatum(m.get("value"))) for m in a.get("metadata", [])]),
        "native": [native(s["native"]) for s in scripts if "native" in s],
        "plutus": nmap(list(plutus.items())),
    }


def body(c, t, top, schema):
    """The ledger body shape of the fields a top level transaction or a sub transaction carries."""
    v = c.get("validity") or {}
    b = {
        "inputs": nset([txin(i) for i in c.get("inputs", [])]),
        "reference_inputs": nset([txin(i) for i in c.get("referenceInputs", [])]),
        "outputs": [output(o) for o in c.get("outputs", [])],
        "certs": noset([cert(x) for x in c.get("certificates", [])]),
        "withdrawals": nmap([(h(w.get("rewardAccount")), big(w.get("coin")) or "0") for w in c.get("withdrawals", [])]),
        "ttl": opt(v.get("ttl"), num),
        "start": opt(v.get("start"), num),
        "guards": noset([cred(x) for x in c.get("guards", [])]),
        "mint": multi(c.get("mint", [])),
        "script_data_hash": opt(c.get("scriptDataHash"), h),
        "aux_hash": opt(c.get("auxiliaryDataHash"), h),
        "network": opt(c.get("networkId"), num),
        "votes": votes(c.get("votes", [])),
        "proposals": noset([proposal(p) for p in c.get("proposals", [])]),
        "treasury": opt(c.get("treasuryValue"), big),
        "donation": big(c.get("treasuryDonation")) or "0",
        "top_guards": nmap([(cred(g.get("credential")), data(g.get("datum"))) for g in c.get("requiredTopLevelGuards", [])]),
        "deposits": nmap([(h(d.get("rewardAccount")), big(d.get("coin")) or "0") for d in c.get("directDeposits", [])]),
        "balance_intervals": intervals(c.get("accountBalanceIntervals", [])),
    }
    if top:
        col = t.get("collateral") or {}
        b.update(
            fee=big(t.get("fee")) or "0",
            collateral=nset([txin(i) for i in col.get("collateral", [])]),
            collateral_return=output(col.get("collateralReturn")),
            total_collateral=opt(col.get("totalCollateral"), big),
            sub_txs=nomap(
                [
                    (h(s.get("hash")), {"body": body(s, None, False, schema), "wits": wits(s.get("witnesses")), "aux": aux(s.get("auxiliary"))})
                    for s in t.get("subTransactions", [])
                ]
            ),
            starting_balance_intervals=intervals(t.get("startingAccountBalanceIntervals", [])),
        )
    return b


def tx(t, schema):
    c = t.get("common", {}) if schema == "v1" else t
    return {
        "body": body(c, t, True, schema),
        "wits": wits(c.get("witnesses")),
        "valid": bool(t.get("successful", False)),
        "aux": aux(c.get("auxiliary")),
    }


def block(b, schema):
    hd = b.get("header", {})
    bd = b.get("body", {})
    eb = hd.get("endorserBlockAnnouncement")
    lc = bd.get("leiosCertificate")
    return {
        "header": {
            "slot": num(hd.get("slot")),
            "hash": h(hd.get("hash")),
            "block_no": num(hd.get("height")),
            "eb": None if eb is None else [h(eb.get("referencesHash")), num(eb.get("referencesSize"))],
            "leios_cert": bool(hd.get("bodyContainsLeiosCertificate", False)),
        },
        "txs": [tx(t, schema) for t in bd.get("tx", [])],
        "leios_cert": None if lc is None else [h(lc.get("signers")), h(lc.get("signature"))],
        "peras_cert": opt(bd.get("perasCertificate"), h),
    }


def schema_value(kind, msg, schema):
    if kind == "ppu":
        return ppu(msg)
    if kind == "block":
        return block(msg, schema)
    t = tx(msg, schema)
    return {"tx": t, "body": t["body"], "wits": t["wits"]}[kind]


# The path tables. A row names a ledger path prefix and the schema field it is compared with.
# A ledger path under no row, or under a row's exclusions, is listed as "no field".

TX_V1 = [
    ("body.inputs", "common.inputs[] txHash outputIndex"),
    ("body.reference_inputs", "common.referenceInputs[] txHash outputIndex"),
    ("body.outputs", "common.outputs[] address coin assets datum script"),
    ("body.certs", "common.certificates[] by variant, relays without their kind"),
    ("body.withdrawals", "common.withdrawals[] rewardAccount coin"),
    ("body.ttl", "common.validity.ttl"),
    ("body.start", "common.validity.start"),
    ("body.fee", "fee"),
    ("body.collateral", "collateral.collateral[]"),
    ("body.collateral_return", "collateral.collateralReturn"),
    ("body.total_collateral", "collateral.totalCollateral"),
    ("body.guards", "common.guards[]"),
    ("body.mint", "common.mint[]"),
    ("body.script_data_hash", "common.scriptDataHash"),
    ("body.aux_hash", "common.auxiliaryDataHash"),
    ("body.network", "common.networkId"),
    ("body.votes", "common.votes[]"),
    ("body.proposals", "common.proposals[] deposit rewardAccount govAction anchor"),
    ("body.treasury", "common.treasuryValue"),
    ("body.donation", "common.treasuryDonation"),
    ("body.sub_txs", "subTransactions[] keyed by hash, each a TxCommon"),
    ("body.top_guards", "common.requiredTopLevelGuards[] credential datum"),
    ("body.deposits", "common.directDeposits[] rewardAccount coin"),
    ("body.balance_intervals", "common.accountBalanceIntervals[]"),
    ("body.starting_balance_intervals", "startingAccountBalanceIntervals[]"),
    ("wits.vkeys", "common.witnesses.vkeywitness[]"),
    ("wits.boots", "common.witnesses.bootstrapWitnesses[]"),
    ("wits.scripts", "common.witnesses.script[], compared as a set of scripts"),
    ("wits.datums", "common.witnesses.plutusDatums[], compared as a set of data"),
    ("wits.redeemers", "common.witnesses.redeemers[] purpose index payload exUnits"),
    ("valid", "successful"),
    ("aux", "common.auxiliary metadata[] scripts[]"),
]

TX_FLAT = [(p, s.replace("common.", "")) for p, s in TX_V1]
V1BETA_ABSENT = {
    "body.guards",
    "body.script_data_hash",
    "body.aux_hash",
    "body.network",
    "body.treasury",
    "body.donation",
    "body.sub_txs",
    "body.top_guards",
    "body.deposits",
    "body.balance_intervals",
    "body.starting_balance_intervals",
}
V1ALPHA_ABSENT = V1BETA_ABSENT | {"body.votes", "wits.boots", "wits.redeemers"}

BLOCK_V1 = [
    ("header.slot", "header.slot"),
    ("header.hash", "header.hash"),
    ("header.block_no", "header.height"),
    ("header.eb", "header.endorserBlockAnnouncement referencesHash referencesSize"),
    ("header.leios_cert", "header.bodyContainsLeiosCertificate"),
    ("leios_cert", "body.leiosCertificate signers signature"),
    ("peras_cert", "body.perasCertificate"),
]


COST_MODELS = [("<18><0>", "costModels.plutusV1"), ("<18><1>", "costModels.plutusV2"), ("<18><2>", "costModels.plutusV3"), ("<18><3>", "costModels.plutusV4")]


def ppu_rows(schema):
    rows = [(f"<{k}>", field) for k, field, _, carried in PPU if schema in carried and k != 18]
    return rows + COST_MODELS


def tables():
    out = {}
    for s in SCHEMAS:
        absent = {"v1": set(), "v1beta": V1BETA_ABSENT, "v1alpha": V1ALPHA_ABSENT}[s]
        txrows = [r for r in (TX_V1 if s == "v1" else TX_FLAT) if r[0] not in absent]
        excl = [(p, f"no {s} field for v1 {f}") for p, f in TX_V1 if p in absent]
        if s != "v1":
            excl += [("body.certs[].bls", "no blsKey in PoolRegistrationCert")]
        # A parameter change proposal holds a whole update, so its keys follow the update table.
        prow = [(f"body.proposals[].action[]{p}", f) for p, f in ppu_rows(s)]
        pex = [
            ("body.proposals[].action[]<", "an update key the schema has no field for"),
            ("body.proposals[].action[]<18>", "a cost model language the schema has no field for"),
        ]
        if s == "v1":
            # A sub transaction body holds proposals of its own.
            sub = "body.sub_txs{}.v."
            prow += [(sub + p, f) for p, f in prow]
            pex += [(sub + p, f) for p, f in pex]
        blocks = [r for r in BLOCK_V1 if s == "v1" or r[0] in ("header.slot", "header.hash", "header.block_no")]
        blocks += [("txs[]." + p, "body.tx[]." + f) for p, f in txrows + prow]
        out[s] = {
            "tx": (txrows + prow, excl + pex),
            "body": ([(p[5:], f) for p, f in txrows + prow if p.startswith("body.")], [(p[5:], f) for p, f in excl + pex if p.startswith("body.")]),
            "wits": ([(p[5:], f) for p, f in txrows if p.startswith("wits.")], [(p[5:], f) for p, f in excl if p.startswith("wits.")]),
            "block": (blocks, [("txs[]." + p, f) for p, f in excl + pex] + [(p, f"no {s} field for v1 {f}") for p, f in BLOCK_V1 if (p, f) not in blocks]),
            "ppu": (
                ppu_rows(s),
                [("<", "an update key the schema has no field for"), ("<18>", "a cost model language the schema has no field for")],
            ),
        }
    return out


def under(path, prefix):
    return path == prefix or (path.startswith(prefix) and (prefix.endswith("<") or path[len(prefix)] in ".[{<"))


def covered(gpath, rows, excl):
    """The row that covers a generic ledger path, preferring the longest prefix, or None."""
    best, blen = None, -1
    for p, f in rows:
        if under(gpath, p) and len(p) > blen:
            best, blen = ("row", p), len(p)
    for p, f in excl:
        if under(gpath, p) and len(p) > blen:
            best, blen = None, len(p)
    return best


def uncovered_class(gpath, excl):
    """The exclusion a generic ledger path falls under, or the path itself when no row or exclusion names it."""
    best = None
    for p, f in excl:
        if under(gpath, p) and (best is None or len(p) > len(best)):
            best = p
    return best if best is not None else gpath


# Ledger values in the shape a schema can hold.


def ledger_view(v, schema):
    """The ledger value with the parts no schema carries in the ledger's form projected to what it does carry."""
    if isinstance(v, dict):
        if v.get("tag") == "pool_reg" and isinstance(v.get("relays"), list):
            v = dict(v, relays=[relay_view(r) for r in v["relays"]])
        if set(v) == {"vkeys", "boots", "scripts", "datums", "redeemers"}:
            v = dict(v)
            for k in ("scripts", "datums"):
                if isinstance(v[k], dict) and "map" in v[k]:
                    v[k] = nset([e["v"] for e in v[k]["map"].values()])
        if schema != "v1" and "ttl" in v and "start" in v:
            v = dict(v, ttl=None if v["ttl"] == "0" else v["ttl"], start=None if v["start"] == "0" else v["start"])
        out = {k: ledger_view(x, schema) for k, x in v.items()}
        if schema != "v1" and "m" in out and isinstance(out["m"], dict):
            out["m"] = {k: x for k, x in out["m"].items() if not (x == "0" and k.strip('"').isdigit())}
        return out
    if isinstance(v, list):
        return [ledger_view(x, schema) for x in v]
    return v


def relay_view(r):
    if not isinstance(r, list) or not r:
        return r
    out = {}
    if r[0] == "0" and len(r) == 4:
        port, ip4, ip6 = r[1], r[2], r[3]
        if ip4 is not None:
            out["ip4"] = ip4
        if ip6 is not None:
            out["ip6"] = ip6
    elif r[0] == "1" and len(r) == 3:
        port, out["dns"] = r[1], r[2]
    elif r[0] == "2" and len(r) == 2:
        port, out["dns"] = None, r[1]
    else:
        return r
    if port is not None:
        out["port"] = port
    return out


def schema_view(v, schema):
    """The schema value with proto3 defaults read as absent, as the ledger view reads them."""
    if isinstance(v, dict):
        out = {k: schema_view(x, schema) for k, x in v.items()}
        if schema != "v1" and "ttl" in out and "start" in out:
            out["ttl"] = None if out["ttl"] in ("0", None) else out["ttl"]
            out["start"] = None if out["start"] in ("0", None) else out["start"]
        if schema != "v1" and "m" in out and isinstance(out["m"], dict):
            out["m"] = {k: x for k, x in out["m"].items() if not (x == "0" and k.strip('"').isdigit())}
        return out
    if isinstance(v, list):
        return [schema_view(x, schema) for x in v]
    return v


def load_lines(path):
    with open(path) as fh:
        return [json.loads(x) for x in fh if x.strip()]


ABSENT = "<absent>"


def cost(a, b):
    fa, fb = flatten(a), flatten(b)
    return len(set(fa.items()) ^ set(fb.items()))


def pair(ls, ss):
    """Pairs the unmatched ledger and schema entries of one collection, each with its closest counterpart."""
    costs = sorted((cost(lv, sv), i, j) for i, (_, lv) in enumerate(ls) for j, (_, sv) in enumerate(ss))
    li, sj, pairs = set(), set(), []
    for _, i, j in costs:
        if i in li or j in sj:
            continue
        li.add(i)
        sj.add(j)
        pairs.append((ls[i][0], ls[i][1], ss[j][0], ss[j][1]))
    pairs += [(k, v, None, None) for i, (k, v) in enumerate(ls) if i not in li]
    pairs += [(None, None, k, v) for j, (k, v) in enumerate(ss) if j not in sj]
    return pairs


def align(ls, ss):
    """Pairs ordered set entries by position, skipping a repeated schema entry only while the schema has more entries left than the ledger."""
    out, i, j, seen = [], 0, 0, set()
    while i < len(ls) or j < len(ss):
        if j < len(ss):
            c = canon(ss[j])
            if c in seen and len(ss) - j > len(ls) - i:
                j += 1
                continue
            seen.add(c)
        out.append((i, ls[i] if i < len(ls) else ABSENT, ss[j] if j < len(ss) else ABSENT))
        i += 1
        j += 1
    return out


class Diffs(list):
    """The differences of one comparison, and whether a native script holding a guard differs as a whole."""

    def __init__(self, guard_whole):
        super().__init__()
        self.guard_whole = guard_whole


def sdiff(lv, sv, path, out):
    """Appends (path, ledger part, schema part) for each place the two values differ, pairing collection entries by value."""
    if lv == sv:
        return
    if isinstance(lv, dict) and isinstance(sv, dict):
        for kind in ("set", "map", "omap", "m"):
            if kind in lv and kind in sv and isinstance(lv[kind], dict) and isinstance(sv[kind], dict):
                la, sa = lv[kind], sv[kind]
                common = set(la) & set(sa)
                for k in common:
                    sdiff(la[k], sa[k], f"{path}{seg(kind, k)}", out)
                ls = [(k, la[k]) for k in la if k not in common and "#" not in k]
                ss = [(k, sa[k]) for k in sa if k not in common and "#" not in k]
                # A repeat stands for an unpaired ledger member only while the ledger has more of them left.
                repeats = [(k, sa[k]) for k in sa if "#" in k]
                ss += repeats[: max(0, len(ls) - len(ss))]
                for lk, lx, sk, sx in pair(ls, ss):
                    if lk is None:
                        out.append((f"{path}{seg(kind, sk)}", ABSENT, sx))
                    elif sk is None:
                        out.append((f"{path}{seg(kind, lk)}", lx, ABSENT))
                    elif kind == "m":
                        out.append((f"{path}{seg(kind, lk)}.key", json.loads(lk), json.loads(sk)))
                        sdiff(lx, sx, f"{path}{seg(kind, lk)}", out)
                    else:
                        sdiff(lx, sx, f"{path}{seg(kind, lk)}", out)
                if kind == "omap":
                    rename = {}
                    for lk, _, sk, _ in pair(ls, ss):
                        if lk is not None and sk is not None:
                            rename[sk] = lk
                    sdiff(lv.get("order"), [rename.get(k, k) for k in sv.get("order", [])], f"{path}.order", out)
                return
        if out.guard_whole and lv.get("tag") in NATIVE_TAGS and holds_guard(lv):
            # A native script holding a guard differs as a whole where a schema has no guard member.
            out.append((path, lv, sv))
            return
        if isinstance(lv.get("oset"), list) and isinstance(sv.get("oset"), list):
            for i, lx, sx in align(lv["oset"], sv["oset"]):
                sdiff(lx, sx, f"{path}[{i}]", out)
            return
        if "tag" in lv and "tag" in sv and lv["tag"] != sv["tag"]:
            out.append((path, lv, sv))
            return
        for k in sorted(set(lv) | set(sv)):
            sdiff(lv.get(k, ABSENT), sv.get(k, ABSENT), f"{path}.{k}" if path else k, out)
        return
    if isinstance(lv, list) and isinstance(sv, list):
        for i in range(max(len(lv), len(sv))):
            sdiff(lv[i] if i < len(lv) else ABSENT, sv[i] if i < len(sv) else ABSENT, f"{path}[{i}]", out)
        return
    out.append((path, lv, sv))


NATIVE_TAGS = {"sig", "all", "any", "m_of", "start", "expire", "guard"}


def holds_guard(v):
    if isinstance(v, dict):
        return v.get("tag") == "guard" or any(holds_guard(x) for x in v.values())
    if isinstance(v, list):
        return any(holds_guard(x) for x in v)
    return False


def seg(kind, k):
    return segment(kind, k) if kind == "m" else f"{{{k}}}"


QUOTED_INT = re.compile(r'\\*"(-?\d+)\\*"')
INT32 = (-(2**31), 2**31 - 1)
UINT32 = (0, 2**32 - 1)
INT64 = (-(2**63), 2**63 - 1)


def outside(x, bounds):
    try:
        v = int(x)
    except (TypeError, ValueError):
        return False
    return not bounds[0] <= v <= bounds[1]


def rational_outside(x):
    if not (isinstance(x, str) and x.startswith("r:")):
        return False
    n, d = x[2:].split("/")
    return outside(n, INT32) or outside(d, UINT32)


def any_leaf(v, pred):
    return any(pred(x) for x in flatten(v).values()) if isinstance(v, (dict, list)) else pred(v)


def listed(schema, gpath, lv, sv):
    """The listed class of one difference, either a schema type that cannot hold the ledger value or a legacy mapping, or None."""
    if any_leaf(lv, rational_outside):
        return "type cannot hold: RationalNumber is int32 over uint32"
    if "aux.metadata" in gpath and any(outside(x, INT64) for x in QUOTED_INT.findall(json.dumps(lv))):
        return "type cannot hold: Metadatum int is int64"
    if gpath == "peras_cert" and lv == "h:" and sv is None:
        return "type cannot hold: empty Peras certificate bytes read as no certificate"
    if gpath.endswith(".dns") and lv == "t:" and sv == ABSENT:
        return "type cannot hold: an empty dns name reads as no name"
    if gpath.endswith(".m") and outside(lv, UINT32):
        return "type cannot hold: ScriptNOfK k is uint32"
    if isinstance(lv, dict) and lv.get("tag") == "m_of" and outside(lv.get("m"), UINT32):
        return "type cannot hold: ScriptNOfK k is uint32"
    if gpath.endswith(".add{}.v") and outside(lv, UINT32):
        return "type cannot hold: NewCommitteeCredentials expires_epoch is uint32"
    if gpath.endswith(".port") and lv == "0" and sv == ABSENT:
        return "type cannot hold: a uint32 port of 0 reads as no port"
    if schema != "v1":
        if any_leaf(lv, lambda x: x == "guard"):
            return "type cannot hold: no guard native script variant"
        if gpath.endswith("redeemers{}.k[]") and lv == "6":
            return "type cannot hold: no guarding redeemer purpose"
        if gpath.endswith("total_collateral") and lv is None and sv == "0":
            return "legacy mapping: no total collateral maps to 0"
        if gpath.endswith("aux") and lv is None and sv == {"metadata": {"map": {}}, "native": [], "plutus": {"map": {}}}:
            return "legacy mapping: no auxiliary data maps to an empty one"
    return None


def result():
    return dict(agree=Counter(), nofield=Counter(), cannot=Counter(), dis=[], compared=0)


def compare_one(kind, schema, ledger, msg, table, r, where):
    """Compares one schema message with one normalized ledger value and adds the outcome to r."""
    rows, excl = table
    r["compared"] += 1
    lview = ledger_view(ledger, schema)
    sview = schema_view(schema_value(kind, msg, schema), schema)
    out = Diffs(guard_whole=schema != "v1")
    sdiff(lview, sview, "", out)
    bad = set()
    for path, lv, sv in out:
        gp = generic(path)
        if covered(gp, rows, excl) is None:
            continue
        bad.add(path)
        why = listed(schema, gp, lv, sv)
        if why:
            r["cannot"][(why, gp)] += 1
            continue
        cls = f"{schema} lacks" if sv == ABSENT else f"{schema} extra" if lv == ABSENT else f"{schema} value differs"
        r["dis"].append(dict(where, cls=cls, key=gp, path=path, ledger=lv, schema=sv))
    for path in flatten(lview):
        gp = generic(path)
        if path.endswith(("[empty]", "{empty}")) and any(p.startswith(generic(path[:-7])) for p, f in rows):
            gp = generic(path[:-7]) + "[empty]"
            if path not in bad:
                r["agree"][gp] += 1
            continue
        if covered(gp, rows, excl) is None:
            r["nofield"][uncovered_class(gp, excl)] += 1
        elif not any(path == b or path.startswith(b) for b in bad):
            r["agree"][gp] += 1


def compare(kind, aligned):
    tabs = tables()
    res = {s: result() for s in SCHEMAS}
    for n, (g, o, p) in enumerate(aligned):
        if not o["ok"] or not p.get("ok"):
            continue
        ln = normalize(o["dump"])
        for s in SCHEMAS:
            m = (p.get("schemas") or {}).get(s, {})
            if not m.get("ok"):
                res[s]["dis"].append(dict(i=n, cls=f"{s} mapper refuses ledger-valid input", key=str(m.get("err"))[:120], hex=g["hex"]))
                continue
            compare_one(kind, s, ln, m["msg"], tabs[s][kind], res[s], dict(i=n, hex=g["hex"]))
    return res


def print_tables():
    for s, kinds in tables().items():
        for kind, (rows, excl) in kinds.items():
            print(f"## {s} {kind}")
            for p, f in rows:
                print(f"  {p}  |  {f}")
            for p, f in excl:
                print(f"  {p}  |  not carried: {f}")


def main():
    if sys.argv[1:] == ["--tables"]:
        print_tables()
        return
    kind, gpath, opath, ppath, prefix = sys.argv[1:6]
    res = compare(kind, rows(gpath, opath, ppath))
    status = 0
    with open(f"{prefix}.schema.dis.jsonl", "w") as fh:
        for s in SCHEMAS:
            for d in res[s]["dis"]:
                fh.write(json.dumps(d, default=str) + "\n")
    with open(f"{prefix}.schema.counts.json", "w") as fh:
        json.dump(
            {
                s: dict(
                    agree=r["agree"],
                    nofield=r["nofield"],
                    cannot={f"{w} {p}": c for (w, p), c in r["cannot"].items()},
                    compared=r["compared"],
                )
                for s, r in res.items()
            },
            fh,
            indent=1,
            sort_keys=True,
        )
    for s in SCHEMAS:
        r = res[s]
        dis = r["dis"]
        print(
            f"schema {s} {kind}: values {r['compared']}, field paths {len(r['agree'])}, leaves agree {sum(r['agree'].values())}, "
            f"ledger classes with no field {len(r['nofield'])}, listed differences {sum(r['cannot'].values())}, "
            f"disagreements {len(dis)} on {len({d['i'] for d in dis})} values"
        )
        for gp in sorted(r["nofield"]):
            print(f"  {r['nofield'][gp]:5d}  listed  no field in {s}  {gp}")
        for (why, gp), c in sorted(r["cannot"].items()):
            print(f"  {c:5d}  listed  {why}  {gp}")
        by = Counter((d["cls"], d.get("key")) for d in dis)
        for (cls, key), c in sorted(by.items(), key=lambda x: (-x[1], str(x[0]))):
            print(f"  {c:5d}  DISAGREE  {cls}  {key or ''}")
        if dis:
            status = 1
    sys.exit(status)


if __name__ == "__main__":
    main()
