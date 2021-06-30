pragma solidity >=0.5.12;

contract SelfDestructContract {
    string public text = "Hello World!";

    function callHelloWorld() public view returns (string memory) {
        return text;
    }

    function callSelfDestruct() public {
        selfdestruct(msg.sender);
    }
}