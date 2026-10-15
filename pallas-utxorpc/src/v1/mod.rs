use std::ops::Deref;

use prost_types::FieldMask;

use pallas_traverse as trv;

use crate::LedgerContext;

pub use utxorpc_spec::utxorpc::v1 as spec;

#[derive(Default, Clone)]
pub struct Mapper<C: LedgerContext> {
    pub(crate) ledger: Option<C>,
    pub(crate) _mask: FieldMask,
}

impl<C: LedgerContext> Mapper<C> {
    pub fn new(ledger: C) -> Self {
        Self {
            ledger: Some(ledger),
            _mask: FieldMask { paths: vec![] },
        }
    }

    /// Creates a clone of this mapper using a custom field mask
    pub fn masked(&self, mask: FieldMask) -> Self {
        Self {
            ledger: self.ledger.clone(),
            _mask: mask,
        }
    }
}

crate::shared::impl_cardano_mapper_shared!(utxorpc_spec::utxorpc::v1::cardano);
crate::since_v1beta::impl_cardano_mapper_since_v1beta!();

/// The message this schema maps a protocol parameter update to.
type ParamUpdate = u5c::PParamsUpdate;

/// The value this schema's update message holds for a scalar key, absent when
/// the update leaves the key unset.
fn update_scalar<T>(value: Option<T>) -> Option<T> {
    value
}

/// Sets the Leios and Peras fields of a mapped block from the block's header
/// and body.
fn with_leios_fields(mut mapped: u5c::Block, block: &trv::MultiEraBlock) -> u5c::Block {
    if true {
        return mapped;
    }
    let header = block.header();

    if let Some(mapped) = mapped.header.as_mut() {
        mapped.body_contains_leios_certificate =
            header.block_body_contains_leios_cert().unwrap_or_default();

        #[cfg(feature = "unstable")]
        {
            mapped.endorser_block_announcement =
                header
                    .eb_announcement()
                    .map(|x| u5c::EndorserBlockAnnouncement {
                        references_hash: x.eb_hash.to_vec().into(),
                        references_size: x.eb_size,
                    });
        }
    }

    #[cfg(feature = "unstable")]
    if let Some(mapped) = mapped.body.as_mut() {
        mapped.leios_certificate = block.leios_certificate().map(|x| u5c::LeiosCertificate {
            signers: x.signers.to_vec().into(),
            signature: x.signature.to_vec().into(),
        });
        mapped.peras_certificate = block
            .peras_certificate()
            .map(|x| x.to_vec().into())
            .unwrap_or_default();
    }

    mapped
}

/// The fields a sub transaction shares, which this schema holds in
/// `Tx.common`.
#[cfg(test)]
fn common(tx: &u5c::Tx) -> &u5c::TxCommon {
    tx.common
        .as_ref()
        .expect("a mapped transaction holds the fields a sub transaction shares")
}

impl<C: LedgerContext> Mapper<C> {
    pub fn map_tx(&self, tx: &trv::MultiEraTx) -> u5c::Tx {
        let resolved = self.ledger.as_ref().and_then(|ctx| {
            let to_resolve = self.find_related_inputs(tx);
            ctx.get_utxos(to_resolve.as_slice())
        });

        u5c::Tx {
            common: Some(self.map_tx_common(tx, &resolved)),
            collateral: self.map_collateral(tx, &resolved),
            fee: tx.fee().and_then(u64_to_bigint),
            successful: tx.is_valid(),
            ..Default::default()
        }
    }

    /// Map the fields a transaction shares with a sub transaction.
    fn map_tx_common(
        &self,
        tx: &trv::MultiEraTx,
        resolved: &Option<crate::UtxoMap>,
    ) -> u5c::TxCommon {
        let start = tx.validity_start();
        let ttl = tx.ttl();

        u5c::TxCommon {
            hash: tx.hash().to_vec().into(),
            inputs: tx
                .inputs_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, i)| self.map_tx_input(i, tx, order as u32, resolved))
                .collect(),
            outputs: tx
                .outputs()
                .iter()
                .map(|x| self.map_tx_output(x, Some(tx)))
                .collect(),
            certificates: tx
                .certs()
                .iter()
                .enumerate()
                .filter_map(|(order, x)| self.map_cert(x, tx, order as u32))
                .collect(),
            proposals: tx
                .gov_proposals()
                .iter()
                .map(|x| self.map_gov_proposal(x))
                .collect(),
            votes: self.map_votes(tx),
            withdrawals: tx
                .withdrawals_sorted_set()
                .iter()
                .enumerate()
                .map(|(order, x)| self.map_withdrawals(x, tx, order as u32))
                .collect(),
            mint: tx
                .mints_sorted_set()
                .iter()
                .map(|x| self.map_policy_assets(x))
                .collect(),
            reference_inputs: tx
                .reference_inputs()
                .iter()
                .map(|x| self.map_tx_reference_input(x, resolved, tx))
                .collect(),
            witnesses: u5c::WitnessSet {
                vkeywitness: tx
                    .vkey_witnesses()
                    .iter()
                    .map(|x| self.map_vkey_witness(x))
                    .collect(),
                script: self.collect_all_scripts(tx),
                plutus_datums: tx
                    .plutus_data()
                    .iter()
                    .map(|x| self.map_plutus_datum(x.deref()))
                    .collect(),
                redeemers: tx
                    .redeemers()
                    .iter()
                    .map(|x| self.map_redeemer(x))
                    .collect(),
                bootstrap_witnesses: tx
                    .bootstrap_witnesses()
                    .iter()
                    .map(|x| self.map_bootstrap_witness(x))
                    .collect(),
            }
            .into(),
            validity: (start.is_some() || ttl.is_some()).then_some(u5c::TxValidity { start, ttl }),
            auxiliary: tx.has_aux_data().then(|| u5c::AuxData {
                metadata: tx
                    .metadata()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|(l, d)| self.map_metadata(l, d))
                    .collect(),
                scripts: self.collect_all_aux_scripts(tx),
            }),
            auxiliary_data_hash: tx
                .auxiliary_data_hash()
                .map(|x| x.to_vec().into())
                .unwrap_or_default(),
            script_data_hash: tx
                .script_data_hash()
                .map(|x| x.to_vec().into())
                .unwrap_or_default(),
            network_id: tx.network_id().map(|x| u8::from(x) as u32),
            treasury_value: tx.treasury_value().and_then(u64_to_bigint),
            treasury_donation: tx.donation().and_then(u64_to_bigint),
            ..Default::default()
        }
    }

    /// Map the collateral fields of a transaction, absent when it writes none
    /// of keys 13, 16 and 17.
    fn map_collateral(
        &self,
        tx: &trv::MultiEraTx,
        resolved: &Option<crate::UtxoMap>,
    ) -> Option<u5c::Collateral> {
        let collateral: Vec<_> = tx
            .collateral()
            .iter()
            .map(|x| self.map_tx_collateral(x, resolved, tx))
            .collect();
        let collateral_return = tx
            .collateral_return()
            .map(|x| self.map_tx_output(&x, Some(tx)));
        let total_collateral = tx.total_collateral().and_then(u64_to_bigint);

        if collateral.is_empty() && collateral_return.is_none() && total_collateral.is_none() {
            return None;
        }

        Some(u5c::Collateral {
            collateral,
            collateral_return,
            total_collateral,
        })
    }
}

/// The block fixtures this schema has a snapshot of, each with the snapshot
/// contents and the snapshot file name.
#[cfg(test)]
fn snapshot_cases() -> Vec<(&'static str, &'static str, &'static str)> {
    #[allow(unused_mut)]
    let mut cases = vec![(
        include_str!("../../../test_data/u5c1.block"),
        include_str!("../../../test_data/u5c_v1.json"),
        "u5c_v1.json",
    )];

    #[cfg(feature = "unstable")]
    cases.push((
        include_str!("../../../test_data/dijkstra6.block"),
        include_str!("../../../test_data/u5c_v1_dijkstra.json"),
        "u5c_v1_dijkstra.json",
    ));

    cases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use pretty_assertions::assert_eq;

    /// The optional fields of a mapped transaction, in the order the
    /// assertions name them.
    type OptionalFields = (
        Option<u5c::TxValidity>,
        bool,
        bool,
        Vec<u8>,
        Vec<u8>,
        Option<u32>,
        Option<u5c::BigInt>,
        Option<u5c::BigInt>,
    );

    /// Reads the optional fields of a mapped transaction.
    fn optional_fields(tx: &u5c::Tx) -> OptionalFields {
        let common = common(tx);
        (
            common.validity.clone(),
            common.auxiliary.is_some(),
            tx.collateral.is_some(),
            common.auxiliary_data_hash.to_vec(),
            common.script_data_hash.to_vec(),
            common.network_id,
            common.treasury_value.clone(),
            common.treasury_donation.clone(),
        )
    }

    #[test]
    fn a_transaction_without_optional_body_keys_maps_none_of_them() {
        let tx = conway_tx_with_certificate_redeemers();
        let mapped = Mapper::new(NoLedger).map_tx(&tx);

        assert_eq!(
            optional_fields(&mapped),
            (None, false, false, vec![], vec![], None, None, None),
            "a body without keys 3, 7, 8, 11, 13, 15, 21 and 22 and with nil auxiliary data maps none of them"
        );
    }

    #[test]
    fn a_transaction_maps_every_optional_body_key_it_carries() {
        let tx = conway_tx_with_optional_body_keys();
        let mapped = Mapper::new(NoLedger).map_tx(&tx);

        assert_eq!(
            optional_fields(&mapped),
            (
                Some(u5c::TxValidity {
                    start: Some(OPTIONAL_VALIDITY_START),
                    ttl: Some(OPTIONAL_TTL),
                }),
                true,
                true,
                OPTIONAL_AUX_DATA_HASH.to_vec(),
                OPTIONAL_SCRIPT_DATA_HASH.to_vec(),
                Some(OPTIONAL_NETWORK_ID as u32),
                u64_to_bigint(OPTIONAL_TREASURY_VALUE),
                u64_to_bigint(OPTIONAL_DONATION),
            ),
            "each key the body writes reaches its field, the network id of 0 and the empty auxiliary data included"
        );
        assert_eq!(
            mapped.collateral.and_then(|x| x.total_collateral),
            u64_to_bigint(OPTIONAL_TOTAL_COLLATERAL),
        );
    }

    #[test]
    fn a_block_before_dijkstra_maps_no_leios_or_peras_field() {
        let cbor = hex::decode(include_str!("../../../test_data/conway1.block")).unwrap();
        let block = trv::MultiEraBlock::decode(&cbor).unwrap();
        let mapped = Mapper::new(NoLedger).map_block(&block);

        let header = mapped.header.unwrap();
        let body = mapped.body.unwrap();
        assert_eq!(
            (
                header.body_contains_leios_certificate,
                header.endorser_block_announcement,
                body.leios_certificate,
                body.peras_certificate.to_vec(),
                mapped.endorser_block,
            ),
            (false, None, None, vec![], None)
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_without_certificates_maps_no_leios_or_peras_field() {
        let block = dijkstra_block(include_str!("../../../test_data/dijkstra11.block"));
        let mapped = Mapper::new(NoLedger).map_block(&block);

        let header = mapped.header.unwrap();
        let body = mapped.body.unwrap();
        assert_eq!(
            (
                header.body_contains_leios_certificate,
                header.endorser_block_announcement,
                body.leios_certificate,
                body.peras_certificate.to_vec(),
            ),
            (false, None, None, vec![])
        );
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_maps_its_leios_certificate_and_announcement() {
        let block = dijkstra_block(include_str!("../../../test_data/dijkstra8.block"));
        let mapped = Mapper::new(NoLedger).map_block(&block);

        let header = mapped.header.unwrap();
        assert!(header.body_contains_leios_certificate);
        assert_eq!(
            header.endorser_block_announcement,
            Some(u5c::EndorserBlockAnnouncement {
                references_hash: hex::decode(
                    "de5f4b812d0e6dc3129510c6663de4ea99bbd6bf019ec2d541853242926fd446"
                )
                .unwrap()
                .into(),
                references_size: 39_495,
            })
        );

        let body = mapped.body.unwrap();
        let certificate = body
            .leios_certificate
            .expect("dijkstra8 carries a Leios certificate");
        assert_eq!(hex::encode(&certificate.signers), "f218a17300000e08200200");
        assert_eq!(certificate.signature.len(), 48);
        assert_eq!(body.peras_certificate.to_vec(), Vec::<u8>::new());
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_certifying_without_announcing_maps_no_announcement() {
        let block = dijkstra_block(include_str!("../../../test_data/dijkstra9.block"));
        let mapped = Mapper::new(NoLedger).map_block(&block);

        let header = mapped.header.unwrap();
        assert!(header.body_contains_leios_certificate);
        assert_eq!(header.endorser_block_announcement, None);
        assert!(mapped.body.unwrap().leios_certificate.is_some());
    }

    #[cfg(feature = "unstable")]
    #[test]
    fn a_dijkstra_block_maps_its_peras_certificate_bytes() {
        use pallas_codec::minicbor;
        use pallas_primitives::dijkstra;

        let cbor = hex::decode(include_str!("../../../test_data/dijkstra3.block")).unwrap();
        let (_, mut block): (u16, dijkstra::Block) = minicbor::decode(&cbor).unwrap();
        block.block_body.peras_certificate =
            pallas_primitives::Nullable::Some(vec![0x22; 16].into());
        let cbor = minicbor::to_vec((8u16, &block)).unwrap();

        let block = trv::MultiEraBlock::decode(&cbor).unwrap();
        let mapped = Mapper::new(NoLedger).map_block(&block);

        assert_eq!(
            mapped.body.unwrap().peras_certificate.to_vec(),
            vec![0x22; 16]
        );
    }
}
