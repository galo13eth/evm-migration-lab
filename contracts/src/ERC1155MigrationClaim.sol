// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { MigrationClaimBase } from "./MigrationClaimBase.sol";
import { MigratedERC1155 } from "./tokens/MigratedERC1155.sol";

contract ERC1155MigrationClaim is MigrationClaimBase {
    constructor(
        bytes32 migrationId_,
        uint256 sourceChainId_,
        address sourceContract_,
        uint256 snapshotBlock_,
        bytes32 sourceBlockHash_,
        uint256 destinationChainId_,
        address destinationToken_,
        uint64 claimStart_,
        uint64 claimDeadline_,
        address owner_
    )
        MigrationClaimBase(
            migrationId_,
            sourceChainId_,
            sourceContract_,
            snapshotBlock_,
            sourceBlockHash_,
            destinationChainId_,
            destinationToken_,
            claimStart_,
            claimDeadline_,
            owner_
        )
    { }

    function _campaignStandard() internal pure override returns (uint8) {
        return 2;
    }

    function _mint(address recipient, uint256 tokenId, uint256 amount) internal override {
        // slither-disable-next-line calls-loop
        MigratedERC1155(destinationToken).mint(recipient, tokenId, amount);
    }
}
