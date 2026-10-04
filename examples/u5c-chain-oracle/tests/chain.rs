use pallas_traverse::MultiEraBlock;
use pallas_utxorpc::v1beta::spec::cardano as u5c;
use u5c_chain_oracle::coverage::{Coverage, EXCLUDED, Location};
use u5c_chain_oracle::effects::{self, Unmapped};
use u5c_chain_oracle::model::AccountOp;
use u5c_chain_oracle::{fields, inner_block, mapper, source};

fn raw(number: u64) -> Vec<u8> {
    let path = format!(
        "{}/fixtures/blocks/{number}.cbor",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn id(text: &str) -> [u8; 32] {
    hex::decode(text)
        .expect("hex")
        .try_into()
        .expect("32 bytes")
}

const C2: &str = "6be37198a765e2078f1e15555bebe1a230a9594ec32821e0f5db0c76cae28386";
const C2_SUB: &str = "729149650a2469af59b1ea8be819c72010d71389683b6a1f0706879c8982d51f";
const A1: &str = "43827b78039ee71003f79eda9cbe50c44e7101195cd7a4e13fba290bbfc11faa";
const N1: &str = "2de11481d8ff00d2195c0801a440d6feb33f23e2278ce98c898769df292dccb2";

/// One chain case block, its u5c mapping and the source parts of each transaction that u5c has no field for.
struct Mapped {
    raw: Vec<u8>,
    u5c: u5c::Block,
    unmapped: Vec<Option<Unmapped>>,
}

fn mapped(number: u64) -> Mapped {
    let raw = raw(number);
    let block = MultiEraBlock::decode(&raw).expect("a block");
    let m = mapper();
    let u5c = m.map_block(&block);
    let unmapped = block
        .as_dijkstra()
        .expect("a Dijkstra block")
        .block_body
        .transactions
        .iter()
        .map(|t| source::unmapped(&m, t))
        .collect();
    Mapped { raw, u5c, unmapped }
}

impl Mapped {
    fn position(&self, tx: &str) -> usize {
        self.txs()
            .iter()
            .position(|t| t.hash.as_ref() == id(tx))
            .unwrap_or_else(|| panic!("{tx} in the block"))
    }

    fn txs(&self) -> &[u5c::Tx] {
        self.u5c.body.as_ref().map_or(&[], |b| b.tx.as_slice())
    }

    fn coverage(&self, number: u64) -> Coverage {
        let mut c = Coverage::default();
        c.add(inner_block(&self.raw), number, &self.u5c);
        c
    }
}

#[test]
fn u5c_carries_the_top_level_id_and_not_the_sub_transaction_id() {
    let m = mapped(104960);
    let block = MultiEraBlock::decode(&m.raw).expect("a block");
    let ids = fields::source_ids(block.as_dijkstra().expect("Dijkstra"), 104960);
    let parent = ids
        .iter()
        .find(|(i, _)| *i == id(C2_SUB))
        .map(|x| x.1.parent);
    assert_eq!(parent, Some(Some(id(C2))));
    let carried = |x: &str| m.txs().iter().any(|t| t.hash.as_ref() == id(x));
    assert!(carried(C2));
    assert!(!carried(C2_SUB));
}

#[test]
fn only_the_source_sub_transaction_produces_the_sub_output() {
    let m = mapped(104960);
    let i = m.position(C2);
    let produced = |unmapped: Option<&Unmapped>| -> Vec<([u8; 32], u32)> {
        effects::applied(&m.txs()[i], 104960, unmapped)
            .iter()
            .flat_map(|fx| fx.produced.iter().map(|(k, _)| *k))
            .collect()
    };
    assert!(produced(m.unmapped[i].as_ref()).contains(&(id(C2_SUB), 0)));
    assert!(!produced(None).contains(&(id(C2_SUB), 0)));
}

#[test]
fn only_the_source_direct_deposit_credits_the_account() {
    let m = mapped(104949);
    let i = m.position(A1);
    let deposits = |unmapped: Option<&Unmapped>| -> Vec<AccountOp> {
        effects::applied(&m.txs()[i], 104949, unmapped)
            .into_iter()
            .flat_map(|fx| fx.accounts)
            .filter(|op| matches!(op, AccountOp::Deposit(..)))
            .collect()
    };
    assert_eq!(deposits(m.unmapped[i].as_ref()).len(), 1);
    assert_eq!(deposits(None), vec![]);
}

#[test]
fn a_failed_transaction_applies_its_collateral_and_not_its_sub_transaction() {
    let m = mapped(105475);
    let i = m.position(N1);
    let tx = &m.txs()[i];
    assert!(!tx.successful);
    let unmapped = m.unmapped[i].as_ref().expect("a sub transaction");
    assert_eq!(unmapped.subs.len(), 1);
    let applied = effects::applied(tx, 105475, Some(unmapped));
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].origin.sub, None);
    assert_eq!(applied[0].produced.len(), 1);
    assert_eq!(applied[0].produced[0].0, (id(N1), tx.outputs.len() as u32));
}

#[test]
fn a_failed_transaction_lists_its_sub_transaction_id_under_it() {
    let raw = raw(105486);
    let block = MultiEraBlock::decode(&raw).expect("a block");
    let ids = fields::source_ids(block.as_dijkstra().expect("Dijkstra"), 105486);
    let n2 = id("5c2a50a1ed150518aa27171e1157a8db6367858a2288629fb00c566c69cb6e68");
    let sub = id("07f6c06348ddd70277fc220b9a7b37d19b3bde80bb4ce04c6eadff4b29348426");
    let parent = ids.iter().find(|(i, _)| *i == sub).map(|x| x.1.parent);
    assert_eq!(parent, Some(Some(n2)));
}

#[test]
fn the_bls_check_finds_a_key_only_where_u5c_carries_it() {
    let m = mapped(4277);
    let block = MultiEraBlock::decode(&m.raw).expect("a block");
    let mut regs = fields::registrations(block.as_dijkstra().expect("Dijkstra"), 4277, &m.u5c);
    let r = regs
        .iter_mut()
        .find(|r| r.bls.is_some())
        .expect("a registration with a BLS key");
    assert!(r.mapped.is_some());
    assert!(!fields::u5c_holds_bls(r));
    let key = r.bls.clone().expect("a key").0;
    r.mapped.as_mut().expect("a mapping").pool_metadata = Some(u5c::PoolMetadata {
        url: String::new(),
        hash: key.into(),
    });
    assert!(fields::u5c_holds_bls(r));
}

fn failing(c: &Coverage) -> Vec<Location> {
    c.failing().into_iter().map(|(l, _)| l).collect()
}

#[test]
fn coverage_holds_the_locations_u5c_maps() {
    let m = mapped(104966);
    let c = m.coverage(104966);
    for l in [
        Location::Body(0),
        Location::Body(1),
        Location::Body(2),
        Location::Wits(0),
        Location::TxPart(3),
        Location::HeaderBody(0),
        Location::HeaderBody(1),
        Location::BlockBody(0),
    ] {
        let t = c
            .tallies
            .get(&l)
            .unwrap_or_else(|| panic!("{l:?} in the block"));
        assert!(t.occurrences > 0 && !t.fails(), "{l:?} {t:?}");
    }
}

#[test]
fn coverage_fails_the_locations_u5c_has_no_field_for() {
    let m = mapped(104966);
    let f = failing(&m.coverage(104966));
    for l in [23, 25, 26, 27] {
        assert!(f.contains(&Location::Body(l)), "{l} in {f:?}");
    }
    assert!(EXCLUDED.iter().all(|(l, _)| !f.contains(l)));
}

#[test]
fn coverage_fails_a_planted_output_drop() {
    let mut m = mapped(104960);
    let before = failing(&m.coverage(104960));
    assert!(!before.contains(&Location::Body(1)), "{before:?}");
    let tx = &mut m.u5c.body.as_mut().expect("a body").tx[0];
    assert!(!tx.outputs.is_empty());
    tx.outputs.clear();
    let after = m.coverage(104960);
    assert_eq!(after.tallies[&Location::Body(1)].disagreements, 1);
    assert!(failing(&after).contains(&Location::Body(1)));
}
