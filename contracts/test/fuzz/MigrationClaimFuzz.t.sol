// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";

import { ERC1155MigrationClaim } from "../../src/ERC1155MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";

contract MigrationClaimFuzzTest is Test {
    ERC1155MigrationClaim private claimContract;
    IMigrationClaim.ClaimData private validClaim;
    bytes32[] private emptyProof;
    address private owner = makeAddr("owner");

    function setUp() public {
        MigratedERC1155 token1155 = new MigratedERC1155("", address(this));
        claimContract = new ERC1155MigrationClaim(
            keccak256("fuzz"),
            1,
            makeAddr("source"),
            100,
            keccak256("source-block"),
            block.chainid,
            address(token1155),
            2,
            type(uint64).max,
            address(this)
        );
        token1155.setMinter(address(claimContract));
        validClaim = IMigrationClaim.ClaimData(7, 1, owner, owner, owner, 0);
        claimContract.setRoot(claimContract.hashLeaf(validClaim), keccak256("artifact"), 1);
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
