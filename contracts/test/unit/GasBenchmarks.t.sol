// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";
import { MerkleProof } from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";

import { ERC1155MigrationClaim } from "../../src/ERC1155MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";
import { MockERC1271Wallet } from "../mocks/MockERC1271Wallet.sol";

contract GasBenchmarksTest is Test {
    bytes32 private constant DOMAIN_TYPEHASH = keccak256(
        "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
    );
    ERC1155MigrationClaim private claimContract;
    address private authority = makeAddr("authority");

    function setUp() public {
        vm.warp(1);
        MigratedERC1155 token = new MigratedERC1155("", address(this));
        claimContract = new ERC1155MigrationClaim(
            keccak256("gas"),
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
    }

    function testGasSingleClaim() public {
        vm.pauseGasMetering();
        IMigrationClaim.ClaimData memory data = _data(authority, 0);
        claimContract.setRoot(claimContract.hashLeaf(data), keccak256("artifact"), 1);
        vm.warp(2);
        vm.prank(authority);
        vm.resumeGasMetering();
        claimContract.claim(data, new bytes32[](0));
    }

    function testGasBatchFive() public {
        _benchmarkBatch(5);
    }

    function testGasBatchTwenty() public {
        _benchmarkBatch(20);
    }

    function testGasDelegatedEOA() public {
        vm.pauseGasMetering();
        uint256 key = 0xB0B;
        address signer = vm.addr(key);
        IMigrationClaim.ClaimData memory data = _data(signer, 0);
        bytes32 leaf = claimContract.hashLeaf(data);
        claimContract.setRoot(leaf, keccak256("artifact"), 1);
        vm.warp(2);
        bytes memory signature = _sign(key, leaf, signer);
        vm.resumeGasMetering();
        claimContract.claimDelegated(data, new bytes32[](0), 0, 1 hours, signature);
    }

    function testGasDelegatedERC1271() public {
        vm.pauseGasMetering();
        uint256 key = 0x5AFE;
        MockERC1271Wallet wallet = new MockERC1271Wallet(vm.addr(key));
        IMigrationClaim.ClaimData memory data =
            IMigrationClaim.ClaimData(7, 1, address(wallet), address(wallet), vm.addr(key), 0);
        bytes32 leaf = claimContract.hashLeaf(data);
        claimContract.setRoot(leaf, keccak256("artifact"), 1);
        vm.warp(2);
        bytes memory signature = _sign(key, leaf, vm.addr(key));
        vm.resumeGasMetering();
        claimContract.claimDelegated(data, new bytes32[](0), 0, 1 hours, signature);
    }

    function _benchmarkBatch(uint256 size) private {
        vm.pauseGasMetering();
        IMigrationClaim.ClaimData[] memory data = new IMigrationClaim.ClaimData[](size);
        bytes32[] memory leaves = new bytes32[](size);
        bool[] memory flags = new bool[](size - 1);
        for (uint256 i; i < size; ++i) {
            data[i] = _data(authority, i);
            leaves[i] = claimContract.hashLeaf(data[i]);
            if (i < size - 1) flags[i] = true;
        }
        claimContract.setRoot(
            MerkleProof.processMultiProof(new bytes32[](0), flags, leaves), keccak256("artifact"), 1
        );
        vm.warp(2);
        vm.prank(authority);
        vm.resumeGasMetering();
        claimContract.claimBatch(data, new bytes32[](0), flags);
    }

    function _data(address signer, uint256 index)
        private
        pure
        returns (IMigrationClaim.ClaimData memory)
    {
        return IMigrationClaim.ClaimData(7, 1, signer, signer, signer, index);
    }

    function _sign(uint256 key, bytes32 leaf, address recipient)
        private
        view
        returns (bytes memory)
    {
        bytes32 domainSeparator = keccak256(
            abi.encode(
                DOMAIN_TYPEHASH,
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
                leaf,
                uint64(1),
                recipient,
                uint256(0),
                uint256(1 hours)
            )
        );
        (uint8 v, bytes32 r, bytes32 s) =
            vm.sign(key, keccak256(abi.encodePacked("\x19\x01", domainSeparator, structHash)));
        return abi.encodePacked(r, s, v);
    }
}
