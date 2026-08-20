// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { ERC1155 } from "@openzeppelin/contracts/token/ERC1155/ERC1155.sol";

contract DemoRelics1155 is ERC1155, Ownable {
    error AlreadySeeded();
    error InvalidSeed();

    bool public seeded;

    constructor(address owner_) ERC1155("ipfs://demo-relics/{id}.json") Ownable(owner_) { }

    function seed(address[] calldata recipients, uint256[] calldata ids, uint256[] calldata amounts)
        external
        onlyOwner
    {
        if (seeded) revert AlreadySeeded();
        if (recipients.length != ids.length || ids.length != amounts.length) revert InvalidSeed();
        seeded = true;
        for (uint256 i; i < recipients.length; ++i) {
            _mint(recipients[i], ids[i], amounts[i], "");
        }
    }
}
