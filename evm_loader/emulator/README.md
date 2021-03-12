Utility to emulate method calls of ethereum smart contracts loaded to solana network and show used contracts.
____
input:
./emulator SOLANA_URL EVM_LOADER CONTRACT_ID CALLER_ID DATA
or for local cluster
./emulator EVM_LOADER CONTRACT_ID CALLER_ID DATA

output:
stdout -> json

steerr -> logs
____
exapmle:
```bash
emulator http://localhost:8899 FEWAFJFgqj44urHXGTDi56BDoKKcEspDhENfJi4g9wcf \
0x9bf34c90bb2fe88a21ba2a743bf86e040e51221a 479298440ecc1804f0a8653f870000c8e98cc7b3 \
0xb41d7af4000000000000000000000000a5e8d19dff6388ee4f0e275e19cbe59086942b50
```
raw stdout
```json
{"accounts":[{"address":"0xa5e8d19dff6388ee4f0e275e19cbe59086942b50","new":false,"writable":true},{"address":"0x9bf34c90bb2fe88a21ba2a743bf86e040e51221a","new":false,"writable":true},{"address":"0x479298440ecc1804f0a8653f870000c8e98cc7b3","new":true,"writable":true}]}
```
formated
```json
{
  "accounts": [
    {
      "address": "0xa5e8d19dff6388ee4f0e275e19cbe59086942b50",
      "new": false,
      "writable": true
    },
    {
      "address": "0x9bf34c90bb2fe88a21ba2a743bf86e040e51221a",
      "new": false,
      "writable": true
    },
    {
      "address": "0x479298440ecc1804f0a8653f870000c8e98cc7b3",
      "new": true,
      "writable": true
    }
  ]
}
```