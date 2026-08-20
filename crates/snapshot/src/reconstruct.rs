use std::collections::BTreeMap;

use alloy::primitives::{Address, U256};

use crate::model::{Holding, TokenStandard, Transfer};
use crate::{Result, SnapshotError};

pub fn reconstruct(standard: TokenStandard, transfers: &[Transfer]) -> Result<Vec<Holding>> {
    let mut ordered = transfers.to_vec();
    ordered.sort_by(|a, b| a.position.cmp(&b.position));
    match standard {
        TokenStandard::Erc721 => reconstruct_721(&ordered),
        TokenStandard::Erc1155 => reconstruct_1155(&ordered),
    }
}

fn reconstruct_721(transfers: &[Transfer]) -> Result<Vec<Holding>> {
    let mut owners = BTreeMap::<U256, Address>::new();
    for transfer in transfers {
        if transfer.amount != U256::from(1) {
            return Err(SnapshotError::Reconstruction(format!(
                "ERC-721 token {} had amount {}",
                transfer.token_id, transfer.amount
            )));
        }
        if transfer.from != Address::ZERO {
            let current = owners.get(&transfer.token_id).copied();
            if current != Some(transfer.from) {
                return Err(SnapshotError::Reconstruction(format!(
                    "token {} expected owner {}, reconstructed {:?}",
                    transfer.token_id, transfer.from, current
                )));
            }
            owners.remove(&transfer.token_id);
        } else if owners.contains_key(&transfer.token_id) {
            return Err(SnapshotError::Reconstruction(format!(
                "token {} was minted twice",
                transfer.token_id
            )));
        }
        if transfer.to != Address::ZERO {
            owners.insert(transfer.token_id, transfer.to);
        }
    }

    Ok(owners
        .into_iter()
        .map(|(token_id, owner)| Holding {
            owner,
            token_id,
            amount: U256::from(1),
        })
        .collect())
}

fn reconstruct_1155(transfers: &[Transfer]) -> Result<Vec<Holding>> {
    let mut balances = BTreeMap::<(U256, Address), U256>::new();
    for transfer in transfers {
        if transfer.from != Address::ZERO {
            let key = (transfer.token_id, transfer.from);
            let current = balances.get(&key).copied().unwrap_or_default();
            let next = current.checked_sub(transfer.amount).ok_or_else(|| {
                SnapshotError::Reconstruction(format!(
                    "token {} owner {} transferred {} with balance {}",
                    transfer.token_id, transfer.from, transfer.amount, current
                ))
            })?;
            if next.is_zero() {
                balances.remove(&key);
            } else {
                balances.insert(key, next);
            }
        }
        if transfer.to != Address::ZERO {
            let key = (transfer.token_id, transfer.to);
            let current = balances.get(&key).copied().unwrap_or_default();
            let next = current.checked_add(transfer.amount).ok_or_else(|| {
                SnapshotError::Reconstruction(format!(
                    "token {} owner {} balance overflowed",
                    transfer.token_id, transfer.to
                ))
            })?;
            if !next.is_zero() {
                balances.insert(key, next);
            }
        }
    }

    Ok(balances
        .into_iter()
        .map(|((token_id, owner), amount)| Holding {
            owner,
            token_id,
            amount,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::address;

    use super::*;
    use crate::model::EventPosition;

    fn transfer(order: u64, from: Address, to: Address, id: u64, amount: u64) -> Transfer {
        Transfer {
            position: EventPosition {
                block_number: order,
                transaction_index: 0,
                log_index: 0,
                sub_index: 0,
            },
            from,
            to,
            token_id: U256::from(id),
            amount: U256::from(amount),
        }
    }

    #[test]
    fn rebuilds_721_mint_transfer_and_burn() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let events = vec![
            transfer(3, b, Address::ZERO, 2, 1),
            transfer(1, Address::ZERO, a, 1, 1),
            transfer(2, a, b, 1, 1),
            transfer(1, Address::ZERO, b, 2, 1),
        ];
        let holdings = reconstruct(TokenStandard::Erc721, &events).unwrap();
        assert_eq!(
            holdings,
            vec![Holding {
                owner: b,
                token_id: U256::from(1),
                amount: U256::from(1)
            }]
        );
    }

    #[test]
    fn rebuilds_1155_balance_math() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let events = vec![
            transfer(1, Address::ZERO, a, 7, 10),
            transfer(2, a, b, 7, 4),
            transfer(3, b, Address::ZERO, 7, 1),
        ];
        assert_eq!(
            reconstruct(TokenStandard::Erc1155, &events).unwrap(),
            vec![
                Holding {
                    owner: a,
                    token_id: U256::from(7),
                    amount: U256::from(6)
                },
                Holding {
                    owner: b,
                    token_id: U256::from(7),
                    amount: U256::from(3)
                },
            ]
        );
    }
}
