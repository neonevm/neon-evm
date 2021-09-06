/*
Implements EIP20 token standard: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-20.md
.*/

pragma solidity >=0.5.12;

import "./EIP20Interface.sol";

contract ERC20Wrapper is EIP20Interface {

    string public name;
    uint8 public decimals;
    string public symbol;

    uint256 tokenMint;

    address constant solana = 0x000000000000000000000000000000000000000A;

    constructor(
        uint256 _tokenMint,
        string memory _name,
        uint8 _decimals,
        string memory _symbol
    ) public {
        tokenMint = _tokenMint;
        name = _name;
        decimals = _decimals;
        symbol = _symbol;
    }
    
    function() external {
        bool status;
        bytes memory result;
        (status, result) = solana.call(msg.data);
        if (!status) {
	  revert();
	}
    }
}
