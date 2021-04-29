pragma solidity ^0.5.12;
       
    
contract Create_Receiver {
    event Foo(address caller, uint amount, string message);

    function foo(string memory _message, uint _x) public payable returns (uint) {
        emit Foo(msg.sender, msg.value, _message);
        return _x + 1;
    }
}

contract Create_Caller {
    event Result_foo(uint r);
    function creator() public payable {
        address addr;
        bytes memory bytecode = type(Create_Receiver).creationCode;
        uint256 salt = 0;
        assembly {
            addr := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        uint result = Create_Receiver(addr).foo("call foo", 123);
        emit Result_foo(result);
    }
}

