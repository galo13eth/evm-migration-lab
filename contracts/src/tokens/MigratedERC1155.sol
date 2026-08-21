// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { Ownable2Step } from "@openzeppelin/contracts/access/Ownable2Step.sol";
import { ERC1155 } from "@openzeppelin/contracts/token/ERC1155/ERC1155.sol";

contract MigratedERC1155 is ERC1155, Ownable2Step {
    error InvalidMinter();
    error MinterAlreadyLocked();
    error MetadataAlreadyFrozen();
    error OnlyMinter(address caller);

    event MinterLocked(address indexed minter);
    event MetadataFrozen();

    address public minter;
    bool public minterLocked;
    bool public metadataFrozen;

    constructor(string memory uri_, address owner_) ERC1155(uri_) Ownable(owner_) { }

    function setMinter(address minter_) external onlyOwner {
        if (minterLocked) revert MinterAlreadyLocked();
        if (minter_ == address(0)) revert InvalidMinter();
        minter = minter_;
        minterLocked = true;
        emit MinterLocked(minter_);
    }

    function setBaseURI(string calldata uri_) external onlyOwner {
        if (metadataFrozen) revert MetadataAlreadyFrozen();
        _setURI(uri_);
    }

    function freezeMetadata() external onlyOwner {
        if (metadataFrozen) revert MetadataAlreadyFrozen();
        metadataFrozen = true;
        emit MetadataFrozen();
    }

    function mint(address to, uint256 tokenId, uint256 amount) external {
        if (msg.sender != minter) revert OnlyMinter(msg.sender);
        _mint(to, tokenId, amount, "");
    }
}
