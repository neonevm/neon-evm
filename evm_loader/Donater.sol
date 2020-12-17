pragma solidity ^0.5.12;

import './ERC20Wrapper.sol';

contract Donater {
    function donate(address wrapper) public {
        ERC20Wrapper e = ERC20Wrapper(wrapper);
        return e.transfer(0x0000000000000000000000000000000000000001, 5);
    }
    
    function donateFrom(address wrapper) public {
        ERC20Wrapper e = ERC20Wrapper(wrapper);
        return e.transferFrom(0x0000000000000000000000000000000000000002,
            0x0000000000000000000000000000000000000001, 5);
    }
}