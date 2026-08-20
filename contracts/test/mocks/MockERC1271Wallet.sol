// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { ECDSA } from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import { IERC1271 } from "@openzeppelin/contracts/interfaces/IERC1271.sol";

contract MockERC1271Wallet is IERC1271 {
    address public immutable signer;

    constructor(address signer_) {
        signer = signer_;
    }

    function isValidSignature(bytes32 hash, bytes memory signature)
        external
        view
        returns (bytes4)
    {
        return ECDSA.recover(hash, signature) == signer
            ? IERC1271.isValidSignature.selector
            : bytes4(0);
    }
}
