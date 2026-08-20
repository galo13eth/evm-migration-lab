// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script, console2 } from "forge-std/Script.sol";

import { DemoRelics721 } from "../src/demo/DemoRelics721.sol";
import { DemoRelics1155 } from "../src/demo/DemoRelics1155.sol";

contract DeploySource is Script {
    function run() external returns (DemoRelics721 relics721, DemoRelics1155 relics1155) {
        address deployer = vm.envAddress("DEPLOYER_ADDRESS");
        address ownerA = vm.envAddress("DEMO_OWNER_A");
        address ownerB = vm.envAddress("DEMO_OWNER_B");
        address ownerC = vm.envAddress("DEMO_OWNER_C");

        address[] memory owners721 = new address[](4);
        owners721[0] = ownerA;
        owners721[1] = ownerA;
        owners721[2] = ownerB;
        owners721[3] = ownerC;

        address[] memory owners1155 = new address[](4);
        owners1155[0] = ownerA;
        owners1155[1] = ownerB;
        owners1155[2] = ownerB;
        owners1155[3] = ownerC;

        uint256[] memory ids = new uint256[](4);
        ids[0] = 7;
        ids[1] = 7;
        ids[2] = 8;
        ids[3] = 9;

        uint256[] memory amounts = new uint256[](4);
        amounts[0] = 3;
        amounts[1] = 5;
        amounts[2] = 2;
        amounts[3] = 11;

        vm.startBroadcast();
        relics721 = new DemoRelics721(deployer);
        relics1155 = new DemoRelics1155(deployer);
        relics721.seed(owners721);
        relics1155.seed(owners1155, ids, amounts);
        vm.stopBroadcast();

        console2.log("DemoRelics721", address(relics721));
        console2.log("DemoRelics1155", address(relics1155));
    }
}
