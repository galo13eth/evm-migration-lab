// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";

import { MigrationClaim } from "../../src/MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC721 } from "../../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";

contract MigrationClaimFuzzTest is Test {
    MigrationClaim private claimContract;
    IMigrationClaim.ClaimData private validClaim;
    bytes32[] private emptyProof;
    address private owner = makeAddr("owner");

    function setUp() public {
        MigratedERC721 token721 = new MigratedERC721("Migrated", "MIG", "", address(this));
        MigratedERC1155 token1155 = new MigratedERC1155("", address(this));
        claimContract = new MigrationClaim(
            keccak256("fuzz"),
            1,
            makeAddr("source"),
            100,
            address(token721),
            address(token1155),
            1,
            type(uint64).max,
            address(this)
        );
        token721.setMinter(address(claimContract));
        token1155.setMinter(address(claimContract));
        validClaim = IMigrationClaim.ClaimData(2, 7, 1, owner, owner, 0);
        claimContract.setRoot(claimContract.hashLeaf(validClaim), 1);
        vm.warp(2);
    }

    function testFuzzMutatedTokenIdNeverVerifies(uint256 tokenId) public {
        vm.assume(tokenId != validClaim.tokenId);
        IMigrationClaim.ClaimData memory mutated = validClaim;
        mutated.tokenId = tokenId;
        vm.prank(owner);
        vm.expectRevert(IMigrationClaim.InvalidProof.selector);
        claimContract.claim(mutated, emptyProof);
    }

    function testFuzz1155AmountBounds(uint256 amount) public {
        IMigrationClaim.ClaimData memory mutated = validClaim;
        mutated.amount = amount;
        vm.prank(owner);
        if (amount == 0) {
            vm.expectRevert(
                abi.encodeWithSelector(IMigrationClaim.InvalidAmount.selector, 2, amount)
            );
        } else if (amount != 1) {
            vm.expectRevert(IMigrationClaim.InvalidProof.selector);
        }
        claimContract.claim(mutated, emptyProof);
    }

    function testFuzzDelegatedNonceAndDeadline(uint256 nonce, uint256 deadline) public {
        nonce = bound(nonce, 1, type(uint128).max);
        deadline = bound(deadline, 0, block.timestamp - 1);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.ExpiredSignature.selector, deadline, block.timestamp
            )
        );
        claimContract.claimDelegated(validClaim, emptyProof, nonce, deadline, "");
    }
}
