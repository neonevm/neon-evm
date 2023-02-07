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

    function createErc20ForSpl(bytes32 _mint) public returns (address erc20spl) {

        require(getErc20ForSpl[_mint] == address(0), 'ERC20 SPL Factory: ERC20_SPL_EXISTS');

        bytes memory bytecode = type(ERC20ForSpl).creationCode;
        bytecode = abi.encodePacked(bytecode, abi.encode(_mint));
        bytes32 salt = keccak256(abi.encodePacked(_mint));
        assembly {
            erc20spl := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        require(erc20spl != address(0), 'ERC20 SPL Factory: SPL TOKEN IS NOT CREATED');

        getErc20ForSpl[_mint] = erc20spl;
        allErc20ForSpl.push(erc20spl);

        emit ERC20ForSplCreated(_mint, erc20spl, allErc20ForSpl.length);
    }

    function createErc20ForSplMintable(string memory _name, string memory _symbol, uint8 _decimals, address _mint_authority) public returns (address erc20spl) {

        bytes memory bytecode = type(ERC20ForSplMintable).creationCode;
        bytecode = abi.encodePacked(bytecode, abi.encode(_name, _symbol, _decimals, _mint_authority));
        bytes32 salt = keccak256(abi.encodePacked(bytes32(0)));
        assembly {
            erc20spl := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        require(erc20spl != address(0), 'ERC20 SPL Factory: SPL TOKEN MINTABLE IS NOT CREATED');

        bytes32 _mint = ERC20ForSplMintable(erc20spl).findMintAccount();
        getErc20ForSpl[_mint] = erc20spl;
        allErc20ForSpl.push(erc20spl);

        emit ERC20ForSplCreated(_mint, erc20spl, allErc20ForSpl.length);
    }
}
