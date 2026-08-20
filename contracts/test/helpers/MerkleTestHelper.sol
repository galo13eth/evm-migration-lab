// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { MigrationClaim } from "../../src/MigrationClaim.sol";
import { IMigrationClaim } from "../../src/interfaces/IMigrationClaim.sol";

abstract contract MerkleTestHelper {
    function _pair(bytes32 a, bytes32 b) internal pure returns (bytes32) {
        return a < b ? keccak256(bytes.concat(a, b)) : keccak256(bytes.concat(b, a));
    }

    function _root(bytes32[4] memory leaves) internal pure returns (bytes32) {
        return _pair(_pair(leaves[0], leaves[1]), _pair(leaves[2], leaves[3]));
    }

    function _proof(bytes32[4] memory leaves, uint256 index)
        internal
        pure
        returns (bytes32[] memory proof)
    {
        proof = new bytes32[](2);
        proof[0] = leaves[index ^ 1];
        proof[1] = index < 2 ? _pair(leaves[2], leaves[3]) : _pair(leaves[0], leaves[1]);
    }

    function _leaf(MigrationClaim claim, IMigrationClaim.ClaimData memory data)
        internal
        view
        returns (bytes32)
    {
        return claim.hashLeaf(data);
    }
}
