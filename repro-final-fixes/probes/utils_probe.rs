
#[cfg(test)]
mod review_probe_helpers {
    use super::*;

    #[test]
    fn review_probe_reward_account_helpers() {
        let script = [vec![0xf1u8], vec![7u8; 28]].concat();
        for extra in [None, Some(vec![0xf1u8, 0x00])] {
            let mut map: BTreeMap<RewardAccount, Coin> = BTreeMap::new();
            map.insert(Bytes::from(script.clone()), 1);
            if let Some(extra) = &extra {
                map.insert(Bytes::from(extra.clone()), 1);
            }
            let refusing = withdrawal_scripts(
                Some(&map),
                ValidationError::PostAlonzo(PostAlonzoError::InputDecoding),
            )
            .map(|found| found.len());
            println!(
                "PROBE undecodable={} withdrawal_scripts={refusing:?}",
                extra.is_some()
            );
        }
    }
}
