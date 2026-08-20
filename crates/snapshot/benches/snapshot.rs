use alloy::primitives::{Address, U256, address};
use criterion::{Criterion, criterion_group, criterion_main};
use evm_snapshot::model::{EventPosition, TokenStandard, Transfer};
use evm_snapshot::reconstruct;

fn reconstruction(c: &mut Criterion) {
    let owner = address!("0000000000000000000000000000000000000001");
    let transfers = (0..10_000)
        .map(|index| Transfer {
            position: EventPosition {
                block_number: index,
                transaction_index: 0,
                log_index: 0,
                sub_index: 0,
            },
            from: Address::ZERO,
            to: owner,
            token_id: U256::from(index),
            amount: U256::from(1),
        })
        .collect::<Vec<_>>();
    c.bench_function("reconstruct 10k ERC-1155 mints", |b| {
        b.iter(|| reconstruct::reconstruct(TokenStandard::Erc1155, &transfers).unwrap());
    });
}

criterion_group!(benches, reconstruction);
criterion_main!(benches);
