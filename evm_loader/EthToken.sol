pragma solidity >=0.5.12;

contract EthToken {
    
    function checkCallerBalance(uint256 balance) public view {
        require(msg.sender.balance == balance);
    }

    function checkContractBalance(uint256 balance) public view {
        require(address(this).balance == balance);
    }

    function retrieve(uint256 amount) public {
        address payable sender = payable(msg.sender);
        sender.transfer(amount);
    }

    function nop() public payable { }
}
