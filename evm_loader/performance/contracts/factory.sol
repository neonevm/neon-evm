pragma solidity =0.5.16;
import './ERC20.sol';

contract Factory{
    event Address(address a);
    event Hash (bytes32 a);

    function get_hash() public {
        bytes memory bytecode = type(ERC20).creationCode;
        bytes32 hash = keccak256(abi.encodePacked(bytecode));
        emit Hash(hash);
    }

    function create_erc20(bytes32 salt) public {
        address addr;
        bytes memory bytecode = type(ERC20).creationCode;
        // bytes32 salt = keccak256(abi.encodePacked(a));

        assembly {
            addr := create2(0, add(bytecode, 32), mload(bytecode), salt)
            if iszero(extcodesize(addr)) {
                revert(0, 0)
            }
        }
        emit Address(addr);
    }
}
