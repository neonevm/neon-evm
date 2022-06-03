#!/bin/bash
set -xeuo pipefail

if ! (wait-for-neon.sh 20 \
      && create-test-accounts.sh 2); then
  echo "Failed to start evm_loader tests"
  exit 1
fi

echo "Deploy test..."
ACCOUNT=$(solana address --keypair /root/.config/solana/id.json)
ACCOUNT2=$(solana address --keypair /root/.config/solana/id2.json)
export ETH_TOKEN_MINT=$(solana address -k neon_token_keypair.json)
export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
export $(neon-cli --evm_loader "$EVM_LOADER" neon-elf-params evm_loader.so)

TOKEN_ACCOUNT=$(spl-token create-account $ETH_TOKEN_MINT --owner $ACCOUNT | grep -Po 'Creating account \K[^\n]*')
spl-token mint $ETH_TOKEN_MINT 5000 --owner evm_loader-keypair.json -- $TOKEN_ACCOUNT
spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT

TOKEN_ACCOUNT2=$(spl-token create-account $ETH_TOKEN_MINT --owner $ACCOUNT2 | grep -Po 'Creating account \K[^\n]*')
spl-token mint $ETH_TOKEN_MINT 5000 --owner evm_loader-keypair.json -- $TOKEN_ACCOUNT2
spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT2

# python3 -m unittest discover -v -p 'test*.py'

echo "Deploy test success"
exit 0
