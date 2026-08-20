// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { StdInvariant } from "forge-std/StdInvariant.sol";
import { Test } from "forge-std/Test.sol";

import { MigrationClaim } from "../../src/MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC721 } from "../../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";

contract ClaimHandler is Test {
    MigrationClaim public immutable claimContract;
    MigratedERC1155 public immutable token;
    address public immutable owner;
    IMigrationClaim.ClaimData private _claim;
    uint256 public countAtPause;
    uint256 public balanceAtPause;

    constructor(
        MigrationClaim claimContract_,
        MigratedERC1155 token_,
        address owner_,
        IMigrationClaim.ClaimData memory claim_
    ) {
        claimContract = claimContract_;
        token = token_;
        owner = owner_;
        _claim = claim_;
    }

    function claim() external {
        vm.prank(owner);
        try claimContract.claim(_claim, new bytes32[](0)) { } catch { }
    }

    function pause() external {
        try claimContract.pause() {
            countAtPause = claimContract.claimedCount();
            balanceAtPause = token.balanceOf(owner, 42);
        } catch { }
    }

    function unpause() external {
        try claimContract.unpause() { } catch { }
    }
}

contract MigrationClaimInvariantTest is StdInvariant, Test {
    MigrationClaim private claimContract;
    MigratedERC1155 private token;
    ClaimHandler private handler;
    address private owner = makeAddr("manifest-owner");

    function setUp() public {
        MigratedERC721 token721 = new MigratedERC721("Migrated", "MIG", "", address(this));
        token = new MigratedERC1155("", address(this));
        claimContract = new MigrationClaim(
            keccak256("invariant"),
            1,
            makeAddr("source"),
            100,
            address(token721),
            address(token),
            1,
            type(uint64).max,
            address(this)
        );
        token721.setMinter(address(claimContract));
        token.setMinter(address(claimContract));
        IMigrationClaim.ClaimData memory data = IMigrationClaim.ClaimData(2, 42, 3, owner, owner, 0);
        claimContract.setRoot(claimContract.hashLeaf(data), 1);
        vm.warp(2);

        handler = new ClaimHandler(claimContract, token, owner, data);
        claimContract.transferOwnership(address(handler));
        vm.prank(address(handler));
        claimContract.acceptOwnership();
        targetContract(address(handler));
    }

    function invariantMintedAmountMatchesClaimBit() public view {
        uint256 count = claimContract.claimedCount();
        assertEq(token.balanceOf(owner, 42), count * 3);
        assertEq(claimContract.isClaimed(1, 0), count == 1);
    }

    function invariantNeverExceedsManifestSize() public view {
        assertLe(claimContract.claimedCount(), 1);
    }

    function invariantPausedStateDoesNotChange() public view {
        if (claimContract.paused()) {
            assertEq(claimContract.claimedCount(), handler.countAtPause());
            assertEq(token.balanceOf(owner, 42), handler.balanceAtPause());
        }
    }
}
