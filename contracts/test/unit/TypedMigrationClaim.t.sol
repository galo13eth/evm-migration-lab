// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";

import { ERC1155MigrationClaim } from "../../src/ERC1155MigrationClaim.sol";
import { ERC721MigrationClaim } from "../../src/ERC721MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC721 } from "../../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";
import { MockERC1271Wallet } from "../mocks/MockERC1271Wallet.sol";

contract TypedMigrationClaimTest is Test {
    bytes32 private constant MIGRATION_ID = keccak256("typed-campaign");
    bytes32 private constant SOURCE_BLOCK_HASH = keccak256("source-block");
    bytes32 private constant ARTIFACT_DIGEST = keccak256("artifact-bundle");
    bytes32 private constant EIP712_DOMAIN_TYPEHASH = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    );
    uint64 private constant START = 1_000;
    uint64 private constant DEADLINE = 10_000;

    address private holder = makeAddr("holder");
    MigratedERC721 private token;
    ERC721MigrationClaim private claimContract;

    function setUp() public {
        vm.warp(START - 1);
        token = new MigratedERC721("Migrated", "MIG", "", address(this));
        claimContract = new ERC721MigrationClaim(
            MIGRATION_ID,
            11_155_111,
            makeAddr("source"),
            123_456,
            SOURCE_BLOCK_HASH,
            block.chainid,
            address(token),
            START,
            DEADLINE,
            address(this)
        );
        token.setMinter(address(claimContract));
    }

    function testERC721CampaignMintsOnlyItsStandard() public {
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, holder, holder, 0);
        claimContract.setRoot(claimContract.hashLeaf(data), ARTIFACT_DIGEST, 1);
        vm.warp(START);

        vm.prank(holder);
        claimContract.claim(data, new bytes32[](0));

        assertEq(token.ownerOf(42), holder);
        assertEq(claimContract.campaignStandard(), 1);
        assertEq(claimContract.destinationToken(), address(token));
    }

    function testERC721CampaignRejectsERC1155Leaf() public {
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, holder, holder, 0);
        bytes32 erc1155Leaf = _leaf(data, 2);
        claimContract.setRoot(erc1155Leaf, ARTIFACT_DIGEST, 1);
        vm.warp(START);

        vm.prank(holder);
        vm.expectRevert(IMigrationClaim.InvalidProof.selector);
        claimContract.claim(data, new bytes32[](0));
    }

    function testClaimAuthorityCanDifferFromHistoricalOwner() public {
        address authority = makeAddr("destination-authority");
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, authority, holder, 0);
        claimContract.setRoot(claimContract.hashLeaf(data), ARTIFACT_DIGEST, 1);
        vm.warp(START);

        vm.prank(holder);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.UnauthorizedClaimAuthority.selector, authority, holder
            )
        );
        claimContract.claim(data, new bytes32[](0));

        vm.prank(authority);
        claimContract.claim(data, new bytes32[](0));
        assertEq(token.ownerOf(42), holder);
    }

    function testRootCorrectionsStopAtLaunch() public {
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, holder, holder, 0);
        bytes32 firstRoot = claimContract.hashLeaf(data);
        claimContract.setRoot(firstRoot, ARTIFACT_DIGEST, 1);

        vm.expectRevert(abi.encodeWithSelector(IMigrationClaim.UnchangedRoot.selector, firstRoot));
        claimContract.setRoot(firstRoot, keccak256("other-bundle"), 2);

        claimContract.setRoot(keccak256("corrected-root"), keccak256("other-bundle"), 2);
        vm.warp(START);
        vm.expectRevert(IMigrationClaim.RootFrozen.selector);
        claimContract.setRoot(keccak256("late-root"), keccak256("late-bundle"), 3);
    }

    function testDelegatedSignatureIsBoundToRootAndVersion() public {
        uint256 authorityKey = 0xA11CE;
        address authority = vm.addr(authorityKey);
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, authority, holder, 0);
        bytes32 leaf = claimContract.hashLeaf(data);
        claimContract.setRoot(leaf, ARTIFACT_DIGEST, 1);
        uint256 signatureDeadline = DEADLINE - 1;
        bytes memory staleSignature =
            _sign(authorityKey, leaf, leaf, 1, holder, 0, signatureDeadline);

        bytes32 sibling = keccak256("corrected-leaf");
        bytes32 correctedRoot = _hashPair(leaf, sibling);
        claimContract.setRoot(correctedRoot, keccak256("corrected-bundle"), 2);
        vm.warp(START);

        bytes32[] memory proof = new bytes32[](1);
        proof[0] = sibling;
        vm.expectRevert(
            abi.encodeWithSelector(IMigrationClaim.InvalidSignature.selector, authority)
        );
        claimContract.claimDelegated(data, proof, 0, signatureDeadline, staleSignature);
    }

    function testERC1155CampaignMintsOnlyERC1155() public {
        MigratedERC1155 token1155 = new MigratedERC1155("ipfs://{id}", address(this));
        ERC1155MigrationClaim claim1155 = new ERC1155MigrationClaim(
            MIGRATION_ID,
            11_155_111,
            makeAddr("source-1155"),
            123_456,
            SOURCE_BLOCK_HASH,
            block.chainid,
            address(token1155),
            START,
            DEADLINE,
            address(this)
        );
        token1155.setMinter(address(claim1155));
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(7, 5, holder, holder, holder, 0);
        claim1155.setRoot(claim1155.hashLeaf(data), ARTIFACT_DIGEST, 1);
        vm.warp(START);

        vm.prank(holder);
        claim1155.claim(data, new bytes32[](0));

        assertEq(token1155.balanceOf(holder, 7), 5);
        assertEq(claim1155.campaignStandard(), 2);
    }

    function testBatchClaimsAndRejectsForeignAuthority() public {
        IMigrationClaim.ClaimData[] memory batch = new IMigrationClaim.ClaimData[](2);
        batch[0] = IMigrationClaim.ClaimData(1, 1, holder, holder, holder, 0);
        batch[1] = IMigrationClaim.ClaimData(2, 1, holder, holder, holder, 1);
        claimContract.setRoot(
            _hashPair(claimContract.hashLeaf(batch[0]), claimContract.hashLeaf(batch[1])),
            ARTIFACT_DIGEST,
            1
        );
        bool[] memory flags = new bool[](1);
        flags[0] = true;
        vm.warp(START);

        vm.prank(holder);
        claimContract.claimBatch(batch, new bytes32[](0), flags);
        assertEq(token.ownerOf(1), holder);
        assertEq(token.ownerOf(2), holder);

        batch[1].claimAuthority = makeAddr("foreign-authority");
        vm.prank(holder);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.UnauthorizedClaimAuthority.selector, batch[1].claimAuthority, holder
            )
        );
        claimContract.claimBatch(batch, new bytes32[](0), flags);
    }

    function testPauseWindowAndDuplicateClaim() public {
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(42, 1, holder, holder, holder, 0);
        claimContract.setRoot(claimContract.hashLeaf(data), ARTIFACT_DIGEST, 1);

        vm.prank(holder);
        vm.expectRevert(
            abi.encodeWithSelector(
                IMigrationClaim.ClaimWindowClosed.selector, START, DEADLINE, START - 1
            )
        );
        claimContract.claim(data, new bytes32[](0));

        vm.warp(START);
        claimContract.pause();
        vm.prank(holder);
        vm.expectRevert();
        claimContract.claim(data, new bytes32[](0));
        claimContract.unpause();
        vm.prank(holder);
        claimContract.claim(data, new bytes32[](0));
        vm.prank(holder);
        vm.expectRevert(abi.encodeWithSelector(IMigrationClaim.AlreadyClaimed.selector, 0));
        claimContract.claim(data, new bytes32[](0));
    }

    function testDelegatedEOAClaim() public {
        uint256 eoaKey = 0xB0B;
        address eoa = vm.addr(eoaKey);
        IMigrationClaim.ClaimData memory eoaData = IMigrationClaim.ClaimData(1, 1, eoa, eoa, eoa, 0);
        bytes32 eoaLeaf = claimContract.hashLeaf(eoaData);
        claimContract.setRoot(eoaLeaf, ARTIFACT_DIGEST, 1);
        vm.warp(START);
        uint256 signatureDeadline = START + 1 hours;

        claimContract.claimDelegated(
            eoaData,
            new bytes32[](0),
            0,
            signatureDeadline,
            _sign(eoaKey, eoaLeaf, eoaLeaf, 1, eoa, 0, signatureDeadline)
        );
        assertEq(token.ownerOf(1), eoa);
        assertEq(claimContract.nonces(eoa), 1);
    }

    function testDelegatedERC1271Claim() public {
        uint256 walletSignerKey = 0x5AFE;
        MockERC1271Wallet wallet = new MockERC1271Wallet(vm.addr(walletSignerKey));
        IMigrationClaim.ClaimData memory walletData =
            IMigrationClaim.ClaimData(2, 1, address(wallet), address(wallet), address(wallet), 0);
        bytes32 walletLeaf = claimContract.hashLeaf(walletData);
        claimContract.setRoot(walletLeaf, ARTIFACT_DIGEST, 1);
        vm.warp(START);
        uint256 signatureDeadline = START + 1 hours;

        claimContract.claimDelegated(
            walletData,
            new bytes32[](0),
            0,
            signatureDeadline,
            _sign(walletSignerKey, walletLeaf, walletLeaf, 1, address(wallet), 0, signatureDeadline)
        );
        assertEq(token.ownerOf(2), address(wallet));
    }

    function _leaf(IMigrationClaim.ClaimData memory data, uint8 standard)
        private
        view
        returns (bytes32)
    {
        return keccak256(
            bytes.concat(
                keccak256(
                    abi.encode(
                        MIGRATION_ID,
                        uint256(11_155_111),
                        claimContract.sourceContract(),
                        uint256(123_456),
                        SOURCE_BLOCK_HASH,
                        block.chainid,
                        standard,
                        data.tokenId,
                        data.amount,
                        data.sourceOwner,
                        data.claimAuthority,
                        data.destinationRecipient,
                        data.leafIndex
                    )
                )
            )
        );
    }

    function _sign(
        uint256 privateKey,
        bytes32 leaf,
        bytes32 root,
        uint64 version,
        address recipient,
        uint256 nonce,
        uint256 deadline
    ) private view returns (bytes memory) {
        bytes32 domainSeparator = keccak256(
            abi.encode(
                EIP712_DOMAIN_TYPEHASH,
                keccak256("EVM Migration Claim"),
                keccak256("2"),
                block.chainid,
                address(claimContract)
            )
        );
        bytes32 structHash = keccak256(
            abi.encode(
                claimContract.DELEGATED_CLAIM_TYPEHASH(),
                leaf,
                root,
                version,
                recipient,
                nonce,
                deadline
            )
        );
        bytes32 digest = keccak256(abi.encodePacked("\x19\x01", domainSeparator, structHash));
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, digest);
        return abi.encodePacked(r, s, v);
    }

    function _hashPair(bytes32 left, bytes32 right) private pure returns (bytes32) {
        return left < right
            ? keccak256(bytes.concat(left, right))
            : keccak256(bytes.concat(right, left));
    }
}
