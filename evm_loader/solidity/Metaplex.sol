// SPDX-License-Identifier: MIT

pragma solidity >= 0.7.0;
pragma abicoder v2;

interface Metaplex {
    function createMetadata(bytes32 _mint, string memory _name, string memory _symbol, string memory _uri) external returns(bytes32);
    function createMasterEdition(bytes32 mint, uint64 maxSupply) external returns(bytes32);

    function isInitialized(bytes32 mint) external view returns(bool);
    function isNFT(bytes32 mint) external view returns(bool);
    function uri(bytes32 mint) external view returns(string memory);
    function name(bytes32 mint) external view returns(string memory);
    function symbol(bytes32 mint) external view returns(string memory);
}