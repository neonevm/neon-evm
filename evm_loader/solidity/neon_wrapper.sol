// SPDX-License-Identifier: MIT
pragma solidity >=0.7.6;

interface INeonWithdraw {
    function withdraw(bytes32) external payable returns(bool);
}

contract NeonToken {
    INeonWithdraw constant NeonPrecompiled = INeonWithdraw(0xFF00000000000000000000000000000000000003);

    function withdraw(bytes32 spender) external payable {
        NeonPrecompiled.withdraw{value: msg.value}(spender);
    }
}