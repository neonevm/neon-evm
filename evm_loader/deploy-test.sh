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
# Parse deployed contract address from output of solana-cli:
# Example output: `Program Id: 853qJy1Z8hfgHe194fVrYUbVDfx88ny7phSCHc481Fc6`
# EVM_LOADER will be empty if the match fails.
export EVM_LOADER=$(solana deploy evm_loader.so | sed '/Program Id: \([0-9A-Za-z]\+\)/,${s//\1/;b};s/^.*$//;$q1')
if [ ${#EVM_LOADER} -eq 0 ]; then
  echo  "EVM_LOADER is not deployed"
  exit 1
fi

python3 -m unittest discover -v -p 'test*.py'

echo "Deploy test success"
exit 0
