// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { StdInvariant } from "forge-std/StdInvariant.sol";
import { Test } from "forge-std/Test.sol";

import { ERC1155MigrationClaim } from "../../src/ERC1155MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";

contract ClaimHandler is Test {
    ERC1155MigrationClaim public immutable claimContract;
    MigratedERC1155 public immutable token;
    IMigrationClaim.ClaimData[2] private _claims;
    bytes32[2] private _leaves;
    uint256 public claimCalls;
    uint256 public successfulClaims;
    uint256 public countAtPause;
    uint256 public balanceAAtPause;
    uint256 public balanceBAtPause;

    constructor(
        ERC1155MigrationClaim claimContract_,
        MigratedERC1155 token_,
        IMigrationClaim.ClaimData memory claimA_,
        IMigrationClaim.ClaimData memory claimB_,
        bytes32 leafA_,
        bytes32 leafB_
    ) {
        claimContract = claimContract_;
        token = token_;
        _claims[0] = claimA_;
        _claims[1] = claimB_;
        _leaves[0] = leafA_;
        _leaves[1] = leafB_;
    }

    function claim(uint256 seed) external {
        uint256 index = seed % 2;
        IMigrationClaim.ClaimData memory data = _claims[index];
        bytes32[] memory proof = new bytes32[](1);
        proof[0] = _leaves[1 - index];
        ++claimCalls;
        vm.prank(data.claimAuthority);
        try claimContract.claim(data, proof) {
            ++successfulClaims;
        } catch { }
    }

    function pause() external {
        try claimContract.pause() {
            countAtPause = claimContract.claimedCount();
            balanceAAtPause = token.balanceOf(_claims[0].destinationRecipient, 42);
            balanceBAtPause = token.balanceOf(_claims[1].destinationRecipient, 7);
        } catch { }
    }

    function unpause() external {
        try claimContract.unpause() { } catch { }
    }
}

contract MigrationClaimInvariantTest is StdInvariant, Test {
    ERC1155MigrationClaim private claimContract;
    MigratedERC1155 private token;
    ClaimHandler private handler;
    address private ownerA = makeAddr("manifest-owner-a");
    address private ownerB = makeAddr("manifest-owner-b");

    function setUp() public {
        token = new MigratedERC1155("", address(this));
        claimContract = new ERC1155MigrationClaim(
            keccak256("invariant"),
            1,
            makeAddr("source"),
            100,
            keccak256("source-block"),
            block.chainid,
            address(token),
            2,
            type(uint64).max,
            address(this)
        );
        token.setMinter(address(claimContract));
        IMigrationClaim.ClaimData memory claimA =
            IMigrationClaim.ClaimData(42, 3, ownerA, ownerA, ownerA, 0);
        IMigrationClaim.ClaimData memory claimB =
            IMigrationClaim.ClaimData(7, 5, ownerB, ownerB, ownerB, 1);
        bytes32 leafA = claimContract.hashLeaf(claimA);
        bytes32 leafB = claimContract.hashLeaf(claimB);
        claimContract.setRoot(
            keccak256(bytes.concat(leafA < leafB ? leafA : leafB, leafA < leafB ? leafB : leafA)),
            keccak256("artifact"),
            1
        );
        vm.warp(2);

        handler = new ClaimHandler(claimContract, token, claimA, claimB, leafA, leafB);
        handler.claim(0);
        claimContract.transferOwnership(address(handler));
        vm.prank(address(handler));
        claimContract.acceptOwnership();
        targetContract(address(handler));
    }

    function invariantMintedAmountsMatchClaimBits() public view {
        bool claimedA = claimContract.isClaimed(0);
        bool claimedB = claimContract.isClaimed(1);
        assertEq(token.balanceOf(ownerA, 42), claimedA ? 3 : 0);
        assertEq(token.balanceOf(ownerB, 7), claimedB ? 5 : 0);
        assertEq(claimContract.claimedCount(), (claimedA ? 1 : 0) + (claimedB ? 1 : 0));
    }

    function invariantLeafIndicesClaimAtMostOnce() public view {
        assertEq(handler.successfulClaims(), claimContract.claimedCount());
        assertLe(claimContract.claimedCount(), 2);
    }

    function invariantPausedStateDoesNotChange() public view {
        if (claimContract.paused()) {
            assertEq(claimContract.claimedCount(), handler.countAtPause());
            assertEq(token.balanceOf(ownerA, 42), handler.balanceAAtPause());
            assertEq(token.balanceOf(ownerB, 7), handler.balanceBAtPause());
        }
    }

    function invariantClaimPathIsLive() public view {
        assertGt(handler.claimCalls(), 0);
        assertGt(handler.successfulClaims(), 0);
    }
}
