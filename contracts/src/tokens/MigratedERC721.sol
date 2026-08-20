// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { Ownable2Step } from "@openzeppelin/contracts/access/Ownable2Step.sol";
import { ERC721 } from "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract MigratedERC721 is ERC721, Ownable2Step {
    error InvalidMinter();
    error MinterAlreadyLocked();
    error OnlyMinter(address caller);

    event MinterLocked(address indexed minter);

    address public minter;
    bool public minterLocked;
    string private _baseTokenURI;

    constructor(string memory name_, string memory symbol_, string memory baseURI_, address owner_)
        ERC721(name_, symbol_)
        Ownable(owner_)
    {
        _baseTokenURI = baseURI_;
    }

    function setMinter(address minter_) external onlyOwner {
        if (minterLocked) revert MinterAlreadyLocked();
        if (minter_ == address(0)) revert InvalidMinter();
        minter = minter_;
        minterLocked = true;
        emit MinterLocked(minter_);
    }

    function setBaseURI(string calldata baseURI_) external onlyOwner {
        _baseTokenURI = baseURI_;
    }

    function mint(address to, uint256 tokenId) external {
        if (msg.sender != minter) revert OnlyMinter(msg.sender);
        _safeMint(to, tokenId);
    }

    function _baseURI() internal view override returns (string memory) {
        return _baseTokenURI;
    }
}
