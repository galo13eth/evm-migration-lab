use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, Bytes, Signature, U256, keccak256};
use alloy::providers::Provider;
use alloy::sol;
use alloy::sol_types::{SolCall, SolValue};
use serde::{Deserialize, Serialize};

use crate::error::rpc_error;
use crate::model::address_hex;
use crate::{Result, SnapshotError};

sol! {
    struct Eip712Domain {
        bytes32 typeHash;
        bytes32 nameHash;
        bytes32 versionHash;
        uint256 chainId;
        address verifyingContract;
    }

    struct MigrationAuthorization {
        bytes32 typeHash;
        bytes32 migrationId;
        uint256 sourceChainId;
        address sourceContract;
        uint256 snapshotBlock;
        bytes32 sourceBlockHash;
        uint256 destinationChainId;
        address claimAuthority;
        address destinationRecipient;
    }

    #[sol(rpc)]
    interface IERC1271 {
        function isValidSignature(bytes32 hash, bytes signature) external view returns (bytes4);
    }
}

const DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const AUTHORIZATION_TYPE: &str = "MigrationAuthorization(bytes32 migrationId,uint256 sourceChainId,address sourceContract,uint256 snapshotBlock,bytes32 sourceBlockHash,uint256 destinationChainId,address claimAuthority,address destinationRecipient)";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthorizationFile {
    pub format: String,
    pub authorizations: Vec<Authorization>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    pub source_owner: String,
    pub claim_authority: String,
    pub destination_recipient: String,
    pub signature: String,
}

pub struct AuthorizationDomain {
    pub migration_id: B256,
    pub source_chain_id: u64,
    pub source_contract: Address,
    pub snapshot_block: u64,
    pub source_block_hash: B256,
    pub destination_chain_id: u64,
}

pub struct ResolvedAuthorization {
    pub claim_authority: Address,
    pub destination_recipient: Address,
}

pub fn digest(domain: &AuthorizationDomain, authority: Address, recipient: Address) -> B256 {
    let domain_separator = keccak256(
        Eip712Domain {
            typeHash: keccak256(DOMAIN_TYPE),
            nameHash: keccak256("EVM Migration Snapshot Authorization"),
            versionHash: keccak256("1"),
            chainId: U256::from(domain.source_chain_id),
            verifyingContract: domain.source_contract,
        }
        .abi_encode(),
    );
    let struct_hash = keccak256(
        MigrationAuthorization {
            typeHash: keccak256(AUTHORIZATION_TYPE),
            migrationId: domain.migration_id,
            sourceChainId: U256::from(domain.source_chain_id),
            sourceContract: domain.source_contract,
            snapshotBlock: U256::from(domain.snapshot_block),
            sourceBlockHash: domain.source_block_hash,
            destinationChainId: U256::from(domain.destination_chain_id),
            claimAuthority: authority,
            destinationRecipient: recipient,
        }
        .abi_encode(),
    );
    keccak256(
        [
            b"\x19\x01".as_slice(),
            domain_separator.as_slice(),
            struct_hash.as_slice(),
        ]
        .concat(),
    )
}

pub async fn resolve<P: Provider + Clone>(
    provider: &P,
    domain: &AuthorizationDomain,
    owners: impl IntoIterator<Item = Address>,
    file: Option<AuthorizationFile>,
    rpc_url: &str,
) -> Result<BTreeMap<Address, ResolvedAuthorization>> {
    let mut supplied = BTreeMap::new();
    if let Some(file) = file {
        if file.format != "evm-migration-authorizations-v1" {
            return Err(SnapshotError::Authorization(format!(
                "unsupported authorization format {}",
                file.format
            )));
        }
        for authorization in file.authorizations {
            let owner = parse_address(&authorization.source_owner, "source owner")?;
            if supplied.insert(owner, authorization).is_some() {
                return Err(SnapshotError::Authorization(format!(
                    "duplicate authorization for {}",
                    address_hex(owner)
                )));
            }
        }
    }

    let mut resolved = BTreeMap::new();
    for owner in owners.into_iter().collect::<BTreeSet<_>>() {
        let Some(authorization) = supplied.remove(&owner) else {
            resolved.insert(
                owner,
                ResolvedAuthorization {
                    claim_authority: owner,
                    destination_recipient: owner,
                },
            );
            continue;
        };
        let authority = parse_address(&authorization.claim_authority, "claim authority")?;
        let recipient = parse_address(&authorization.destination_recipient, "recipient")?;
        let signature = Bytes::from_str(&authorization.signature).map_err(|error| {
            SnapshotError::Authorization(format!(
                "invalid signature for {}: {error}",
                address_hex(owner)
            ))
        })?;
        verify(
            provider, domain, owner, authority, recipient, signature, rpc_url,
        )
        .await?;
        resolved.insert(
            owner,
            ResolvedAuthorization {
                claim_authority: authority,
                destination_recipient: recipient,
            },
        );
    }
    if !supplied.is_empty() {
        return Err(SnapshotError::Authorization(format!(
            "authorization supplied for non-holder {}",
            address_hex(*supplied.keys().next().unwrap())
        )));
    }
    Ok(resolved)
}

async fn verify<P: Provider + Clone>(
    provider: &P,
    domain: &AuthorizationDomain,
    owner: Address,
    authority: Address,
    recipient: Address,
    signature: Bytes,
    rpc_url: &str,
) -> Result<()> {
    let digest = digest(domain, authority, recipient);
    let block = BlockId::from(domain.snapshot_block);
    let code = provider
        .get_code_at(owner)
        .block_id(block)
        .await
        .map_err(|error| rpc_error(rpc_url, error))?;
    let valid = if code.is_empty() {
        Signature::try_from(signature.as_ref())
            .and_then(|value| value.recover_address_from_prehash(&digest))
            .is_ok_and(|recovered| recovered == owner)
    } else {
        IERC1271::new(owner, provider.clone())
            .isValidSignature(digest, signature)
            .block(block)
            .call()
            .await
            .map_err(|error| rpc_error(rpc_url, error))?
            == IERC1271::isValidSignatureCall::SELECTOR
    };
    if !valid {
        return Err(SnapshotError::Authorization(format!(
            "invalid source-chain authorization for {}",
            address_hex(owner)
        )));
    }
    Ok(())
}

fn parse_address(value: &str, field: &str) -> Result<Address> {
    value
        .parse()
        .map_err(|error| SnapshotError::Authorization(format!("invalid {field}: {error}")))
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{address, b256};

    use super::*;

    #[test]
    fn authorization_digest_is_domain_separated() {
        let domain = AuthorizationDomain {
            migration_id: b256!("1111111111111111111111111111111111111111111111111111111111111111"),
            source_chain_id: 11_155_111,
            source_contract: address!("0000000000000000000000000000000000000001"),
            snapshot_block: 123_456,
            source_block_hash: b256!(
                "2222222222222222222222222222222222222222222222222222222222222222"
            ),
            destination_chain_id: 84_532,
        };
        assert_ne!(
            digest(
                &domain,
                address!("0000000000000000000000000000000000000002"),
                address!("0000000000000000000000000000000000000003")
            ),
            digest(
                &domain,
                address!("0000000000000000000000000000000000000004"),
                address!("0000000000000000000000000000000000000003")
            )
        );
    }
}
