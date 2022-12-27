// SPDX-License-Identifier: MIT
pragma solidity >=0.5.12;

contract rw_lock {
    mapping(address => mapping(uint256 => uint256)) public data;
    uint len = 0;
    string public text;

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

    function update_storage_str(string memory new_text) public {
        text = new_text;
    }

    function update_storage_map(uint resize) public {
        uint n = 0;
        while (n < resize){
            data[msg.sender][n] = uint256(n);
            n = n + 1;
        }
    }

    function get_text() public view returns (string memory) {
        return text;
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

contract rw_lock_caller {
    rw_lock rw;

    constructor(address rw_lock_address) {
        rw = rw_lock(rw_lock_address);
    }

    function unchange_storage(uint8 x, uint8 y) public view returns(uint8) {
        return rw.unchange_storage(x, y);
    }

    function update_storage_str(string memory new_text) public {
        rw.update_storage_str(new_text);
    }

    function update_storage_map(uint resize) public {
        rw.update_storage_map(resize);
    }

    function get_text() public view returns (string memory) {
        return rw.get_text();
    }
}

