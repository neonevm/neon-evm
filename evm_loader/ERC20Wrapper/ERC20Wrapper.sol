/*
Implements EIP20 token standard: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-20.md
.*/

pragma solidity >=0.5.12;

import "./EIP20Interface.sol";

contract ERC20Wrapper is EIP20Interface {
    address constant solana = 0xFF00000000000000000000000000000000000001;

    string public name;
    uint8 public decimals;
    string public symbol;
    uint256 tokenMint;

    constructor(
	string memory _name,
	uint8 _decimals,
	string memory _symbol,
	uint256 _tokenMint
    ) public {
	name = _name;
	decimals = _decimals;
	symbol = _symbol;
	tokenMint = _tokenMint;
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
