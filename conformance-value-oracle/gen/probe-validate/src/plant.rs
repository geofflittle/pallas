//! Transactions that differ from the base transaction by one value the ledger decoder refuses.

use crate::fixture::{Fixture, REFERENCE_TX, pay};
use crate::term::{self, Term, rational, set};

/// One planted transaction, with the table row it counts in.
pub struct Plant {
    pub kind: &'static str,
    pub variant: String,
    pub tx: Term,
}

const POOL_DEPOSIT: i128 = 500_000_000;
const PROPOSAL_DEPOSIT: i128 = 100_000_000_000;

fn bytes(b: u8, n: usize) -> Term {
    Term::Bytes(vec![b; n])
}

fn header(h: u8, hash: usize) -> Term {
    Term::Bytes([vec![h], vec![0x5a; hash]].concat())
}

fn body(tx: &mut Term) -> &mut Term {
    &mut tx.items_mut()[0]
}

fn own_account(fx: &Fixture) -> Term {
    Term::Bytes([vec![0xe0], fx.key_hash()].concat())
}

struct Pool {
    margin: Term,
    reward_account: Term,
    owners: Vec<Term>,
    bls: Option<Term>,
}

impl Pool {
    fn new(fx: &Fixture) -> Self {
        Pool {
            margin: rational(1, 100),
            reward_account: own_account(fx),
            owners: vec![Term::Bytes(fx.key_hash())],
            bls: None,
        }
    }

    fn certificate(self, fx: &Fixture) -> Term {
        let mut c = vec![Term::Int(3), Term::Bytes(fx.key_hash()), bytes(0x11, 32)];
        c.extend(self.bls);
        c.extend([
            Term::Int(0),
            Term::Int(170_000_000),
            self.margin,
            self.reward_account,
            set(self.owners),
            Term::Array(vec![]),
            Term::Null,
        ]);
        Term::Array(c)
    }
}

fn proposal(action: Term, return_account: Term) -> Term {
    Term::Array(vec![
        Term::Int(PROPOSAL_DEPOSIT),
        return_account,
        action,
        Term::Array(vec![Term::Text("https://example.com".into()), bytes(0x22, 32)]),
    ])
}

fn parameter_change(key: i128, value: Term) -> Term {
    Term::Array(vec![
        Term::Int(0),
        Term::Null,
        Term::Map(vec![(Term::Int(key), value)]),
        Term::Null,
    ])
}

fn info() -> Term {
    Term::Array(vec![Term::Int(6)])
}

fn with_certificates(tx: &mut Term, certificates: Vec<Term>, deposits: i128) {
    let b = body(tx);
    b.set(4, set(certificates));
    pay(b, -deposits);
}

fn with_proposals(tx: &mut Term, proposals: Vec<Term>, deposits: i128) {
    let b = body(tx);
    b.set(20, set(proposals));
    pay(b, -deposits);
}

fn set_output_address(tx: &mut Term, address: Term) {
    let output = &mut body(tx).get_mut(1).unwrap().items_mut()[0];
    match output {
        Term::Array(x) => x[0] = address,
        Term::Map(_) => output.set(0, address),
        other => panic!("not an output: {other:?}"),
    }
}

fn repeat_first(items: &mut Vec<Term>) {
    let first = items[0].clone();
    items.push(first);
}

/// The base transaction and every planted one, each signed by the fixture key.
pub fn plants(fx: &Fixture) -> Vec<Plant> {
    let mut out = Vec::new();
    let mut add = |kind: &'static str, variant: &str, edit: &dyn Fn(&mut Term)| {
        let mut tx = fx.base();
        edit(&mut tx);
        fx.sign(&mut tx);
        out.push(Plant { kind, variant: variant.into(), tx });
    };

    add("base", "unchanged", &|_| ());

    for (kind, variant, margin) in [
        ("zero denominator", "pool margin 1/0", rational(1, 0)),
        ("unit interval above one", "pool margin 3/2", rational(3, 2)),
    ] {
        add(kind, variant, &|tx| {
            let p = Pool { margin: margin.clone(), ..Pool::new(fx) };
            with_certificates(tx, vec![p.certificate(fx)], POOL_DEPOSIT);
        });
    }
    for (kind, variant, key, value) in [
        ("zero denominator", "pledge influence 1/0", 9, rational(1, 0)),
        ("zero denominator", "treasury growth 1/0", 11, rational(1, 0)),
        ("zero denominator", "memory price 1/0", 19, Term::Array(vec![rational(1, 0), rational(1, 1)])),
        ("unit interval above one", "treasury growth 3/2", 11, rational(3, 2)),
        ("unit interval above one", "expansion rate 3/2", 10, rational(3, 2)),
    ] {
        add(kind, variant, &|tx| {
            let p = proposal(parameter_change(key, value.clone()), own_account(fx));
            with_proposals(tx, vec![p], PROPOSAL_DEPOSIT);
        });
    }

    for h in [0x90, 0xa0, 0xb0, 0xc0, 0xd0] {
        add("address header", &format!("output header {h:#04x}"), &|tx| {
            set_output_address(tx, header(h, 28));
        });
    }
    for (variant, address) in [
        ("output enterprise key hash of 27 bytes", header(0x60, 27)),
        ("output enterprise script hash of 27 bytes", header(0x70, 27)),
        ("output base address stake hash of 27 bytes", header(0x00, 28 + 27)),
    ] {
        add("short hash in address", variant, &|tx| set_output_address(tx, address.clone()));
    }
    for (kind, variant, account) in [
        ("address header", "withdrawal account header 0x60", header(0x60, 28)),
        ("address header", "withdrawal account header 0x90", header(0x90, 28)),
        ("short hash in address", "withdrawal account hash of 27 bytes", header(0xe0, 27)),
    ] {
        add(kind, variant, &|tx| {
            body(tx).set(5, Term::Map(vec![(account.clone(), Term::Int(0))]));
        });
    }
    for (kind, variant, account) in [
        ("address header", "pool reward account header 0x60", header(0x60, 28)),
        ("short hash in address", "pool reward account hash of 27 bytes", header(0xe0, 27)),
    ] {
        add(kind, variant, &|tx| {
            let p = Pool { reward_account: account.clone(), ..Pool::new(fx) };
            with_certificates(tx, vec![p.certificate(fx)], POOL_DEPOSIT);
        });
    }
    for (kind, variant, account) in [
        ("address header", "proposal return account header 0x60", header(0x60, 28)),
        ("short hash in address", "proposal return account hash of 27 bytes", header(0xe0, 27)),
    ] {
        add(kind, variant, &|tx| {
            with_proposals(tx, vec![proposal(info(), account.clone())], PROPOSAL_DEPOSIT);
        });
    }

    for (variant, extra) in [("inputs, balanced once", 0), ("inputs, balanced twice", 2_000_000)] {
        add("duplicate set element", variant, &|tx| {
            let b = body(tx);
            let inputs = b.get_mut(0).unwrap().items_mut();
            let first = inputs[0].clone();
            inputs.insert(1, first);
            pay(b, extra);
        });
    }
    add("duplicate set element", "collateral inputs", &|tx| {
        repeat_first(body(tx).get_mut(13).unwrap().items_mut());
    });
    add("duplicate set element", "reference inputs", &|tx| {
        let r = Term::Array(vec![Term::Bytes(REFERENCE_TX.to_vec()), Term::Int(0)]);
        body(tx).set(18, set(vec![r.clone(), r]));
    });
    add("duplicate set element", "guards", &|tx| {
        repeat_first(body(tx).get_mut(14).unwrap().items_mut());
    });
    for (variant, deposits) in [
        ("certificates, one deposit", POOL_DEPOSIT),
        ("certificates, two deposits", 2 * POOL_DEPOSIT),
    ] {
        add("duplicate set element", variant, &|tx| {
            let c = Pool::new(fx).certificate(fx);
            with_certificates(tx, vec![c.clone(), c], deposits);
        });
    }
    add("duplicate set element", "pool owners", &|tx| {
        let owner = Term::Bytes(fx.key_hash());
        let p = Pool { owners: vec![owner.clone(), owner], ..Pool::new(fx) };
        with_certificates(tx, vec![p.certificate(fx)], POOL_DEPOSIT);
    });
    for (variant, deposits) in [
        ("proposals, one deposit", PROPOSAL_DEPOSIT),
        ("proposals, two deposits", 2 * PROPOSAL_DEPOSIT),
    ] {
        add("duplicate set element", variant, &|tx| {
            let p = proposal(info(), own_account(fx));
            with_proposals(tx, vec![p.clone(), p], deposits);
        });
    }
    add("duplicate set element", "Plutus V3 scripts", &|tx| {
        repeat_first(tx.items_mut()[1].get_mut(7).unwrap().items_mut());
    });
    add("duplicate set element", "datums", &|tx| {
        tx.items_mut()[1].set(4, set(vec![Term::Int(1), Term::Int(1)]));
    });
    add("duplicate map key", "withdrawal accounts", &|tx| {
        let a = own_account(fx);
        body(tx).set(5, Term::Map(vec![(a.clone(), Term::Int(0)), (a, Term::Int(0))]));
    });
    add("duplicate map key", "redeemers", &|tx| {
        if let Some(Term::Map(xs)) = tx.items_mut()[1].get_mut(5) {
            let first = xs[0].clone();
            xs.push(first);
        }
    });

    for (variant, key) in [
        ("pool BLS key not a curve point", Term::Array(vec![bytes(0x01, 96), bytes(0x01, 48)])),
        ("pool BLS public key of 95 bytes", Term::Array(vec![bytes(0x01, 95), bytes(0x01, 48)])),
        ("pool BLS proof of 47 bytes", Term::Array(vec![bytes(0x01, 96), bytes(0x01, 47)])),
    ] {
        add("BLS key encoding", variant, &|tx| {
            let p = Pool { bls: Some(key.clone()), ..Pool::new(fx) };
            with_certificates(tx, vec![p.certificate(fx)], POOL_DEPOSIT);
        });
    }

    let mut tx = fx.base();
    repeat_first(tx.items_mut()[1].get_mut(0).unwrap().items_mut());
    out.push(Plant { kind: "duplicate set element", variant: "key witnesses".into(), tx });
    out
}

/// The plants as JSON lines of kind, variant and transaction hex.
pub fn lines(fx: &Fixture) -> Vec<String> {
    plants(fx)
        .into_iter()
        .map(|p| {
            let hex = hex::encode(term::write(&p.tx));
            serde_json::json!({ "kind": p.kind, "variant": p.variant, "hex": hex }).to_string()
        })
        .collect()
}
