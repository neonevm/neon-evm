pragma solidity >=0.5.12;

contract EthToken {
    
    function checkCallerBalance(uint256 balance) public view {
        require(msg.sender.balance == balance);
    }

    function checkUserBalance(address account, uint256 balance) public view {
        require(account.balance == balance);
    }

    function transferTo(address account) public payable {
       address payable target = payable(account);
       target.transfer(msg.value);
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
