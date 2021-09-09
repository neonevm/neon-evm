/*
Implements EIP20 token standard: https://github.com/ethereum/EIPs/blob/master/EIPS/eip-20.md
.*/

pragma solidity >=0.5.12;

contract ERC20Wrapper {
    address constant solana = 0xff00000000000000000000000000000000000001;

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
	bytes memory call_data = abi.encodePacked(tokenMint, msg.data);
	(status, result) = solana.call(call_data);
	if (!status) {
	    revert();
	}
        assembly {
            return(add(result, 0x20), mload(result))
        }
    }
}
