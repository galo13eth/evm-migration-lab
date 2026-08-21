// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script, console2 } from "forge-std/Script.sol";
import { Ownable2Step } from "@openzeppelin/contracts/access/Ownable2Step.sol";

contract AcceptTokenOwnership is Script {
    function run() external {
        address admin = vm.envAddress("ADMIN_ADDRESS");
        Ownable2Step token = Ownable2Step(vm.envAddress("DESTINATION_TOKEN"));
        vm.startBroadcast(admin);
        token.acceptOwnership();
        vm.stopBroadcast();

        console2.log("TokenOwner", token.owner());
        console2.log("TokenPendingOwner", token.pendingOwner());
    }
}
