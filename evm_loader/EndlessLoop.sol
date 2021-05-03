pragma solidity ^0.5.12;

contract EndlessLoop {
    function execute() external pure returns(uint256 res) {
        uint256 cnt=0;

        for(uint256 i = 0; i < uint256(-1); i++) {
            cnt++;
        }

        return cnt;
    }
}

