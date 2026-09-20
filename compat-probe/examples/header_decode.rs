// The behaviour arm. The main probe coerces methods to function pointers, so
// it proves a v1.4.0 caller still COMPILES and is blind to a call that now
// answers differently. This arm asks one such call the same question against
// both trees.
//
// v1.4.0's MultiEraHeader::decode ends in a catch-all that returns
// BabbageCompatible for every envelope tag it does not name, so all five tags
// below answer Ok. A tree that gates the Dijkstra arm but leaves the refusing
// arm ungated answers Err for every tag above 6 on a default build.
//
// Expected verdict against aef0b5a: all five Ok, exit 0.
// Expected verdict against the reworked branch with default features: tags 5
// and 6 Ok, tags 7, 8 and 200 Err, exit 1.
//
// The exit code is the assertion. A run that prints five Ok lines against a
// tree whose decode has no catch-all would mean the tags chosen no longer
// reach the arm, so the tag list is part of what is being pinned.

use pallas_traverse::{MultiEraBlock, MultiEraHeader};

const TAGS: [u8; 5] = [5, 6, 7, 8, 200];

fn main() {
    let raw_block = include_str!("../../test_data/babbage1.block");
    let cbor = hex::decode(raw_block.trim()).expect("the test block is hex");

    // Decoding the block itself must succeed, otherwise a later Err would be
    // reporting a broken fixture rather than the arm under test.
    let block = MultiEraBlock::decode(&cbor).expect("the test block decodes");
    let header = block.header().cbor().to_vec();

    let mut refused = Vec::new();

    for tag in TAGS {
        match MultiEraHeader::decode(tag, None, &header) {
            Ok(h) => println!("  decode(tag={tag}) Ok({:?})", std::mem::discriminant(&h)),
            Err(e) => {
                println!("  decode(tag={tag}) Err({e})");
                refused.push(tag);
            }
        }
    }

    if refused.is_empty() {
        println!("every tag answers Ok, which is the v1.4.0 behaviour");
        return;
    }

    println!(
        "{} of {} tags now refuse: {refused:?}. A v1.4.0 caller that read these headers stops reading them.",
        refused.len(),
        TAGS.len()
    );
    std::process::exit(1);
}
