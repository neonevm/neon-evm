pragma solidity >=0.5.16;

    contract UserTools{
    event Salt(bytes32 a);

    function get_salt(address tokenA, address tokenB) public  {
        require(tokenA != tokenB, 'UniswapV2: IDENTICAL_ADDRESSES');
        (address token0, address token1) = tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
        require(token0 != address(0), 'UniswapV2: ZERO_ADDRESS');
        bytes32 salt = keccak256(abi.encodePacked(token0, token1));
        emit Salt(salt);
    }
}
