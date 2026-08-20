use alloy::primitives::{Address, U256, address};
use evm_snapshot::model::{EventPosition, TokenStandard, Transfer};
use evm_snapshot::reconstruct;
use proptest::prelude::*;

fn transfer(order: u64, from: Address, to: Address, amount: u64) -> Transfer {
    Transfer {
        position: EventPosition {
            block_number: order,
            transaction_index: 0,
            log_index: 0,
            sub_index: 0,
        },
        from,
        to,
        token_id: U256::from(7),
        amount: U256::from(amount),
    }
}

proptest! {
    #[test]
    fn erc1155_transfer_preserves_supply(minted in 1u64..1_000_000, moved in 0u64..1_000_000) {
        let moved = moved.min(minted);
        let alice = address!("0000000000000000000000000000000000000001");
        let bob = address!("0000000000000000000000000000000000000002");
        let events = vec![
            transfer(2, alice, bob, moved),
            transfer(1, Address::ZERO, alice, minted),
        ];
        let holdings = reconstruct::reconstruct(TokenStandard::Erc1155, &events).unwrap();
        let total = holdings.iter().fold(U256::ZERO, |sum, holding| sum + holding.amount);
        prop_assert_eq!(total, U256::from(minted));
    }
}
