// SPDX-License-Identifier: MIT
pragma solidity >=0.5.12;

import './erc20_for_spl.sol';

contract ERC20ForSplFactory {

    mapping(bytes32 => address) public getErc20ForSpl;
    address[] public allErc20ForSpl;

    event ERC20ForSplCreated(bytes32 _mint, address pair, uint);

    function allErc20ForSplLength() external view returns (uint) {
        return allErc20ForSpl.length;
    }

    function createErc20ForSpl(string memory _name, string memory _symbol, bytes32 _mint) public returns (address erc20spl) {

        require(getErc20ForSpl[_mint] == address(0), 'ERC20 SPL Factory: ERC20_SPL_EXISTS');

        bytes memory bytecode = type(ERC20ForSpl).creationCode;
        bytecode = abi.encodePacked(bytecode, abi.encode(_name, _symbol, _mint));
        bytes32 salt = keccak256(abi.encodePacked(_mint));
        assembly {
            erc20spl := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        require(erc20spl != address(0), 'ERC20 SPL Factory: SPL TOKEN IS NOT CREATED');

        getErc20ForSpl[_mint] = erc20spl;
        allErc20ForSpl.push(erc20spl);

        emit ERC20ForSplCreated(_mint, erc20spl, allErc20ForSpl.length);
    }
}
