// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { ERC721 } from "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract DemoRelics721 is ERC721, Ownable {
    error AlreadySeeded();

    bool public seeded;

    constructor(address owner_) ERC721("Demo Relics", "RELIC") Ownable(owner_) { }

    function seed(address[] calldata recipients) external onlyOwner {
        if (seeded) revert AlreadySeeded();
        seeded = true;
        for (uint256 i; i < recipients.length; ++i) {
            _safeMint(recipients[i], i + 1);
        }
    }
}
