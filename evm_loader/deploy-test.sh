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
export EVM_LOADER=$(solana-deploy deploy evm_loader.so | sed '/Program Id:\([0-9A-Za-z]\+\)/,${s//\1/;b};$q1')
python3 -m unittest discover -v -p 'test*.py'

echo "Deploy test success"
exit 0
