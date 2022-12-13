// SPDX-License-Identifier: MIT
pragma solidity >=0.5.12;

contract rw_lock {

    mapping(address => mapping(uint256 => uint256)) data;
    uint len = 0;

    function unchange_storage(uint8 x, uint8 y) public pure returns(uint8) {
        return x + y;
    }

    function update_storage(uint resize) public {
        uint n = 0;

        while (n < resize){
            data[msg.sender][len+n] = uint256(len+n);
            n = n + 1;
        }
        len = len + resize;
    }

    function deploy_contract() public returns(address){
        hello_world hello = new hello_world();
        hello.call_hello_world();
        return address(hello);
    }

}


contract hello_world {
    uint public num = 5;
    string public text = "Hello World!";

    function call_hello_world() public view returns (string memory) {
        return text;
    }
}

contract small {
    function call_hello() public view returns (string memory) {
        return "Hi";
    }
}


contract  string_setter{
    string public text;


    function get() public view returns (string memory) {
        return text;
    }

    function set(string memory new_text) public payable {
        text = new_text;
    }
}
