pragma solidity ^0.5.12;

import './ERC20Wrapper.sol';

contract Donater {
    function donate(address wrapper) public {
        ERC20Wrapper e = ERC20Wrapper(wrapper);
        return e.transfer(true, hex"0000000000000000000000000000000000000001", 5);
    }
    
    function donateFrom(address wrapper) public {
        ERC20Wrapper e = ERC20Wrapper(wrapper);
        return e.transferFrom(
            true, hex"0000000000000000000000000000000000000002",
            true, hex"0000000000000000000000000000000000000001", 5);
    }
}