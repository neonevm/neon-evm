// SPDX-License-Identifier: MIT
pragma solidity >=0.5.12;

interface INeon {
    function withdraw(bytes32 spender) external payable returns (bool);
}

contract NeonToken is INeon {
    address constant NeonPrecompiled = 0xFF00000000000000000000000000000000000003;

    function withdraw(bytes32 spender) public payable returns (bool) {
        return INeon(NeonPrecompiled).withdraw{value: msg.value}(spender);
    }
}