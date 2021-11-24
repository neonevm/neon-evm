#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set -u "$SOLANA_URL"

export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
export $(neon-cli --evm_loader="$EVM_LOADER" neon-elf-params ./evm_loader.so)

export ETH_TOKEN_MINT=$(solana address -k neon_token_keypair.json)
if [ "$ETH_TOKEN_MINT" != "$NEON_TOKEN_MINT" ]; then
  echo "Token address in evm_loader.so is $NEON_TOKEN_MINT"
  echo "Token address in neon_token_keypair.json is  $ETH_TOKEN_MINT"
  echo "Failed to deploy NEON token"
  exit 1
fi

if ! solana account "$ETH_TOKEN_MINT" >/dev/null 2>&1; then
  echo "Creating NEON token $ETH_TOKEN_MINT..."
  if ! spl-token create-token --owner evm_loader-keypair.json -- neon_token_keypair.json; then
    echo "ETH token mint is not created"
    exit 1
  fi
else
  echo "Token $ETH_TOKEN_MINT already exist"
fi

export COLLATERAL_POOL_BASE=$(solana address -k collateral-pool-keypair.json)
if [ "$COLLATERAL_POOL_BASE" != "$NEON_POOL_BASE" ]; then
  echo "Collateral pool address in evm_loader.so is $NEON_POOL_BASE"
  echo "Collateral pool address in collateral-pool-keypair.json is  $COLLATERAL_POOL_BASE"
  echo "Failed to create collateral pool"
  exit 1
fi

echo "Creating collateral pool $NEON_POOL_BASE..."
solana -k collateral-pool-keypair.json airdrop 1000
python3 collateral_pool_generator.py collateral-pool-keypair.json

echo "Deploying evm_loader at address $EVM_LOADER..."
if ! solana program deploy --upgrade-authority evm_loader-keypair.json evm_loader.so >evm_loader_id; then
  echo "Failed to deploy evm_loader"
  exit 1
fi
