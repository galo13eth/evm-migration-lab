// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { IERC1271 } from "@openzeppelin/contracts/interfaces/IERC1271.sol";
import { IERC721Receiver } from "@openzeppelin/contracts/token/ERC721/IERC721Receiver.sol";
import { ERC1155Holder } from "@openzeppelin/contracts/token/ERC1155/utils/ERC1155Holder.sol";
import { ECDSA } from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

contract DemoSourceWallet is IERC1271, IERC721Receiver, ERC1155Holder {
    error InvalidSigner();

    address public immutable signer;

    constructor(address signer_) {
        if (signer_ == address(0)) revert InvalidSigner();
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

    function onERC721Received(address, address, uint256, bytes calldata)
        external
        pure
        returns (bytes4)
    {
        return IERC721Receiver.onERC721Received.selector;
    }
}
