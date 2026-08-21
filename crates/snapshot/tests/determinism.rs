use std::str::FromStr;

use alloy::primitives::{Address, U256, address, b256};
use evm_snapshot::merkle;
use evm_snapshot::model::{
    EventPosition, Manifest, ManifestEntry, TokenStandard, Transfer, address_hex,
};
use evm_snapshot::reconstruct;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureTransfer {
    block_number: u64,
    transaction_index: u64,
    log_index: u64,
    sub_index: u32,
    from: String,
    to: String,
    token_id: u64,
    amount: u64,
}

#[test]
fn fixture_produces_byte_identical_manifest_and_root() {
    let raw = include_str!("../../../fixtures/transfers/erc1155.json");
    let fixtures: Vec<FixtureTransfer> = serde_json::from_str(raw).unwrap();
    let transfers = fixtures
        .into_iter()
        .map(|item| Transfer {
            position: EventPosition {
                block_number: item.block_number,
                transaction_index: item.transaction_index,
                log_index: item.log_index,
                sub_index: item.sub_index,
            },
            from: Address::from_str(&item.from).unwrap(),
            to: Address::from_str(&item.to).unwrap(),
            token_id: U256::from(item.token_id),
            amount: U256::from(item.amount),
        })
        .collect::<Vec<_>>();
    let holdings = reconstruct::reconstruct(TokenStandard::Erc1155, &transfers).unwrap();
    let campaign = merkle::campaign(
        b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        31_337,
        address!("0000000000000000000000000000000000000010"),
        4,
        b256!("2222222222222222222222222222222222222222222222222222222222222222"),
        31_338,
        2,
        "confirmations:0".into(),
    );
    let entries = holdings
        .into_iter()
        .enumerate()
        .map(|(index, holding)| ManifestEntry {
            standard: 2,
            token_id: holding.token_id.to_string(),
            amount: holding.amount.to_string(),
            source_owner: address_hex(holding.owner),
            claim_authority: address_hex(holding.owner),
            destination_recipient: address_hex(holding.owner),
            leaf_index: index as u64,
            leaf_hash: String::new(),
        })
        .collect();
    let manifest = Manifest {
        format: "evm-migration-manifest-v2".into(),
        campaign,
        entries,
    };

    let first = merkle::build(manifest.clone()).unwrap();
    let second = merkle::build(manifest).unwrap();
    assert_eq!(
        serde_json::to_vec_pretty(&first.manifest).unwrap(),
        serde_json::to_vec_pretty(&second.manifest).unwrap()
    );
    assert_eq!(first.proofs.root, second.proofs.root);
    assert_eq!(first.manifest.entries.len(), 2);
}
