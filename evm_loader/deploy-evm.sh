#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set -u "$SOLANA_URL"

export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
export $(neon-cli --evm_loader="$EVM_LOADER" neon-elf-params ./evm_loader.so)

function DeployToken {
  TOKEN_NAME=$1
  NEON_TOKEN_ADDR_VAR=$2
  KEYPAIR_FILE=$3

  echo "Deploying token $TOKEN_NAME..."

  export EXPECTED_VALUE=$(solana address -k "$KEYPAIR_FILE")
  if [ "$EXPECTED_VALUE" != "${!NEON_TOKEN_ADDR_VAR}" ]; then
    echo "Client $TOKEN_NAME address in evm_loader.so is ${!NEON_TOKEN_ADDR_VAR}"
    echo "Expected token address in $KEYPAIR_FILE is $EXPECTED_VALUE"
    echo "Failed to deploy $TOKEN_NAME"
    exit 1
  fi

  if ! solana account "${!NEON_TOKEN_ADDR_VAR}" >/dev/null 2>&1; then
    echo "Creating $TOKEN_NAME ${!NEON_TOKEN_ADDR_VAR}..."
    if ! spl-token create-token --owner evm_loader-keypair.json -- "$KEYPAIR_FILE"; then
      echo "$TOKEN_NAME is not created"
      exit 1
    fi
  else
    echo "Token ${!NEON_TOKEN_ADDR_VAR} already exist"
  fi

  echo "Token $TOKEN_NAME successfully deployed"
}

DeployToken "Client Allowance Token" NEON_CLIENT_ALLOWANCE_TOKEN client_allowance_token_keypair.json
DeployToken "Client Denial Token" NEON_CLIENT_DENIAL_TOKEN client_denial_token_keypair.json
DeployToken "Contract Allowance Token" NEON_CONTRACT_ALLOWANCE_TOKEN contract_allowance_token_keypair.json
DeployToken "Contract Denial Token" NEON_CONTRACT_DENIAL_TOKEN contract_denial_token_keypair.json

echo "Deploying evm_loader at address $EVM_LOADER..."
if ! solana program deploy --upgrade-authority evm_loader-keypair.json evm_loader.so >evm_loader_id; then
  echo "Failed to deploy evm_loader"
  exit 1
fi
sleep 30

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
