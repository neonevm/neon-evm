#!/bin/bash
set -xeuo pipefail

echo "Deploy test..."
[-e evm_loader-deploy_test-net ] || solana-keygen new --no-passphrase
ACCOUNT=$(solana address)

solana config set --url $SOLANA_URL
for i in {1..10}; do
    if solana cluster-version; then break; fi
    sleep 2
done

solana airdrop 1000
solana account $ACCOUNT

echo "Run tests for EVM Loader"
export EVM_LOADER=$(solana deploy evm_loader.so | tail -n 1 | python3 -c 'import sys, json; data=json.load(sys.stdin); print(data["programId"]);')
python3 -m unittest discover -v -p 'test*.py'

echo "Deploy test success"
exit 0
