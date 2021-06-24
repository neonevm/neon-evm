pragma solidity =0.5.16;

import './ERC20.sol';

contract Factory{

    event Address(address a);

    function create_erc20() public returns (address addr){
        bytes memory bytecode = type(ERC20).creationCode;
        assembly {
                addr := create(0, add(bytecode, 0x20), mload(bytecode))
                if iszero(extcodesize(addr)) {
                    revert(0, 0)
                }
        }
        emit Address(addr);
    }
}