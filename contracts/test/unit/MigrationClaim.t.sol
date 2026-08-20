// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";

import { MigrationClaim } from "../../src/MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC721 } from "../../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";
import { MerkleTestHelper } from "../helpers/MerkleTestHelper.sol";
import { MockERC1271Wallet } from "../mocks/MockERC1271Wallet.sol";

contract MigrationClaimTest is Test, MerkleTestHelper {
    bytes32 private constant MIGRATION_ID = keccak256("sepolia-base-sepolia-demo-v1");
    bytes32 private constant EIP712_DOMAIN_TYPEHASH = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    );
    uint64 private constant START = 1_000;
    uint64 private constant DEADLINE = 10_000;

    uint256 private aliceKey = 0xA11CE;
    uint256 private bobKey = 0xB0B;
    uint256 private safeSignerKey = 0x5AFE;
    address private alice;
    address private bob;
    address private safeSigner;
    address private recipientA = makeAddr("recipient-a");
    address private recipientB = makeAddr("recipient-b");

    MigrationClaim private claimContract;
    MigratedERC721 private token721;
    MigratedERC1155 private token1155;
    MockERC1271Wallet private wallet;
    IMigrationClaim.ClaimData[4] private claims;
    bytes32[4] private leaves;

    function setUp() public {
        vm.warp(START);
        alice = vm.addr(aliceKey);
        bob = vm.addr(bobKey);
        safeSigner = vm.addr(safeSignerKey);
        wallet = new MockERC1271Wallet(safeSigner);

        token721 = new MigratedERC721("Migrated Relics", "mRELIC", "ipfs://relic/", address(this));
        token1155 = new MigratedERC1155("ipfs://relic/{id}.json", address(this));
        claimContract = new MigrationClaim(
            MIGRATION_ID,
            11_155_111,
            makeAddr("source"),
            123_456,
            address(token721),
            address(token1155),
            START,
            DEADLINE,
            address(this)
        );
        token721.setMinter(address(claimContract));
        token1155.setMinter(address(claimContract));

        claims[0] = IMigrationClaim.ClaimData(1, 1, 1, alice, recipientA, 0);
        claims[1] = IMigrationClaim.ClaimData(1, 2, 1, alice, recipientB, 1);
        claims[2] = IMigrationClaim.ClaimData(2, 7, 3, bob, recipientA, 2);
        claims[3] = IMigrationClaim.ClaimData(2, 8, 5, address(wallet), recipientB, 3);
        for (uint256 i; i < 4; ++i) {
            leaves[i] = _leaf(claimContract, claims[i]);
        }
        claimContract.setRoot(_root(leaves), 1);
    }

    function testClaimsERC721AndERC1155() public {
        vm.prank(alice);
        claimContract.claim(claims[0], _proof(leaves, 0));
        vm.prank(bob);
        claimContract.claim(claims[2], _proof(leaves, 2));

        assertEq(token721.ownerOf(1), recipientA);
        assertEq(token1155.balanceOf(recipientA, 7), 3);
        assertEq(claimContract.claimedCount(), 2);
    }

    function testRejectsMutatedProofOwnerAndDoubleClaim() public {
        IMigrationClaim.ClaimData memory mutated = claims[0];
        mutated.recipient = bob;
        vm.prank(alice);
        vm.expectRevert(IMigrationClaim.InvalidProof.selector);
        claimContract.claim(mutated, _proof(leaves, 0));

        vm.prank(bob);
        vm.expectRevert(
            abi.encodeWithSelector(IMigrationClaim.UnauthorizedSourceOwner.selector, alice, bob)
        );
        claimContract.claim(claims[0], _proof(leaves, 0));

        vm.startPrank(alice);
        claimContract.claim(claims[0], _proof(leaves, 0));
        vm.expectRevert(abi.encodeWithSelector(IMigrationClaim.AlreadyClaimed.selector, 1, 0));
        claimContract.claim(claims[0], _proof(leaves, 0));
        vm.stopPrank();
    }

    function testWindowAndPause() public {
        vm.warp(START - 1);
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.ClaimWindowClosed.selector, START, DEADLINE, START - 1
            )
        );
        claimContract.claim(claims[0], _proof(leaves, 0));

        vm.warp(START);
        claimContract.pause();
        vm.prank(alice);
        vm.expectRevert();
        claimContract.claim(claims[0], _proof(leaves, 0));

        claimContract.unpause();
        vm.warp(DEADLINE + 1);
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.ClaimWindowClosed.selector, START, DEADLINE, DEADLINE + 1
            )
        );
        claimContract.claim(claims[0], _proof(leaves, 0));
    }

    function testRootVersionScopesClaimedBitmap() public {
        vm.prank(alice);
        claimContract.claim(claims[0], _proof(leaves, 0));
        claimContract.setRoot(_root(leaves), 2);

        assertTrue(claimContract.isClaimed(1, 0));
        assertFalse(claimContract.isClaimed(2, 0));
        assertEq(claimContract.claimedCount(), 0);
    }

    function testBatchMultiproofAndForeignOwnerRejection() public {
        IMigrationClaim.ClaimData[] memory batch = new IMigrationClaim.ClaimData[](2);
        batch[0] = claims[0];
        batch[1] = claims[1];
        bytes32[] memory proof = new bytes32[](1);
        proof[0] = _pair(leaves[2], leaves[3]);
        bool[] memory flags = new bool[](2);
        flags[0] = true;

        vm.prank(alice);
        claimContract.claimBatch(batch, proof, flags);
        assertEq(token721.ownerOf(1), recipientA);
        assertEq(token721.ownerOf(2), recipientB);

        setUp();
        batch[1] = claims[2];
        vm.prank(alice);
        vm.expectRevert(
            abi.encodeWithSelector(IMigrationClaim.UnauthorizedSourceOwner.selector, bob, alice)
        );
        claimContract.claimBatch(batch, proof, flags);
    }

    function testDelegatedEOAAndReplayProtection() public {
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory signature = _sign(aliceKey, claims[0], 0, deadline);

        vm.prank(makeAddr("relayer"));
        claimContract.claimDelegated(claims[0], _proof(leaves, 0), 0, deadline, signature);
        assertEq(token721.ownerOf(1), recipientA);
        assertEq(claimContract.nonces(alice), 1);

        vm.expectRevert(abi.encodeWithSelector(IMigrationClaim.InvalidNonce.selector, alice, 1, 0));
        claimContract.claimDelegated(claims[0], _proof(leaves, 0), 0, deadline, signature);
    }

    function testDelegatedERC1271() public {
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory signature = _sign(safeSignerKey, claims[3], 0, deadline);

        claimContract.claimDelegated(claims[3], _proof(leaves, 3), 0, deadline, signature);
        assertEq(token1155.balanceOf(recipientB, 8), 5);
    }

    function testDelegatedRejectsExpiryAndWrongRecipientSignature() public {
        uint256 expired = block.timestamp - 1;
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.ExpiredSignature.selector, expired, block.timestamp
            )
        );
        claimContract.claimDelegated(claims[0], _proof(leaves, 0), 0, expired, "");

        uint256 deadline = block.timestamp + 1 hours;
        IMigrationClaim.ClaimData memory otherRecipient = claims[0];
        otherRecipient.recipient = bob;
        bytes memory signature = _sign(aliceKey, otherRecipient, 0, deadline);
        vm.expectRevert(abi.encodeWithSelector(IMigrationClaim.InvalidSignature.selector, alice));
        claimContract.claimDelegated(claims[0], _proof(leaves, 0), 0, deadline, signature);
    }

    function _sign(
        uint256 privateKey,
        IMigrationClaim.ClaimData memory data,
        uint256 nonce,
        uint256 deadline
    ) private view returns (bytes memory) {
        bytes32 domainSeparator = keccak256(
            abi.encode(
                EIP712_DOMAIN_TYPEHASH,
                keccak256("EVM Migration Claim"),
                keccak256("1"),
                block.chainid,
                address(claimContract)
            )
        );
        bytes32 structHash = keccak256(
            abi.encode(
                claimContract.DELEGATED_CLAIM_TYPEHASH(),
                _leaf(claimContract, data),
                data.recipient,
                nonce,
                deadline
            )
        );
        bytes32 digest = keccak256(abi.encodePacked("\x19\x01", domainSeparator, structHash));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
        return abi.encodePacked(r, s, v);
    }
}
