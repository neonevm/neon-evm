pragma solidity >=0.5.0;

contract SolidityPrecompilesTest {
    function test_01_ecrecover(bytes32 hash, uint8 v, bytes32 r, bytes32 s) public pure returns (address) {
        return ecrecover(hash, v, r, s);
    }

    function test_02_sha256(bytes memory data) public pure returns (bytes32) {
        return sha256(data);
    }

    function test_03_ripemd160(bytes memory data) public pure returns (bytes20) {
        return ripemd160(data);
    }

    function test_04_dataCopy(bytes memory data) public returns (bytes memory) {
        bytes memory ret = new bytes(data.length);
        assembly {
            let len := mload(data)
            if iszero(call(gas(), 0x04, 0, add(data, 0x20), len, add(ret,0x20), len)) {
                invalid()
            }
        }

        return ret;
    }

    function test_05_bigModExp(bytes32 base, bytes32 exponent, bytes32 modulus) public returns (bytes32 result) {
        assembly {
            // free memory pointer
            let memPtr := mload(0x40)

            // length of base, exponent, modulus
            mstore(memPtr, 0x20)
            mstore(add(memPtr, 0x20), 0x20)
            mstore(add(memPtr, 0x40), 0x20)

            // assign base, exponent, modulus
            mstore(add(memPtr, 0x60), base)
            mstore(add(memPtr, 0x80), exponent)
            mstore(add(memPtr, 0xa0), modulus)

            // call the precompiled contract BigModExp (0x05)
            if iszero(call(gas(), 0x05, 0x0, memPtr, 0xc0, memPtr, 0x20)) {
                revert(0x0, 0x0)
            }
            result := mload(memPtr)
        }
    }

    function test_06_bn256Add(bytes memory input) public returns (bytes32[2] memory result) {
        assembly {
            let len := mload(input)
            if iszero(call(gas(), 0x06, 0, add(input, 0x20), len, result, 0x40)) {
                revert(0,0)
            }
        }
    }

    function test_07_bn256ScalarMul(bytes memory input) public returns (bytes32[2] memory result) {
        assembly {
            let len := mload(input)
            if iszero(call(gas(), 0x07, 0, add(input, 0x20), len, result, 0x40)) {
                revert(0,0)
            }
        }
    }

    function test_08_bn256Pairing(bytes memory input) public returns (bytes32 result) {
        // input is a serialized bytes stream of (a1, b1, a2, b2, ..., ak, bk) from (G_1 x G_2)^k
        uint256 len = input.length;
        assembly {
            let memPtr := mload(0x40)
            if iszero(call(gas(), 0x08, 0, add(input, 0x20), len, memPtr, 0x20)) {
                revert(0,0)
            }
            result := mload(memPtr)
        }
    }

    function test_09_blake2F(uint32 rounds, bytes32[2] memory h, bytes32[4] memory m, bytes8[2] memory t, bool f) public returns (bytes32[2] memory) {
        bytes32[2] memory output;

        bytes memory args = abi.encodePacked(rounds, h[0], h[1], m[0], m[1], m[2], m[3], t[0], t[1], f);

        assembly {
            if iszero(call(gas(), 0x09, 0, add(args, 32), 0xd5, output, 0x40)) {
                revert(0, 0)
            }
        }

        return output;
    }
}
