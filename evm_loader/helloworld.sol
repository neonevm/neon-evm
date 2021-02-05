pragma solidity ^0.5.12;

contract helloWorld {
    string public text = "Hello World!";

    function callHelloWorld() public view returns (string memory) {
        return text;
    }
}
