#!/bin/bash
set -euo pipefail
set +v

echo "Deploy test..."

export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
echo EVM_LOADER=${EVM_LOADER}

echo "Wait for NeonEVM"
wait-for-neon.sh 240

ELF_PARAMS=$(neon-cli --evm_loader "$EVM_LOADER" neon-elf-params evm_loader.so)
echo ${ELF_PARAMS}
export $(python3 -c "
import sys, json
for key, value in json.loads(sys.argv[1])['value'].items():
   print(f'{key}={value}')
" "$ELF_PARAMS")

echo "Create test operator accounts"
create-test-accounts.sh 2

py.test -vvvvv -n 16 tests/

echo "Deploy test success"
exit 0
