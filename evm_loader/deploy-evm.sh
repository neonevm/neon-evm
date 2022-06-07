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
    echo "$TOKEN_NAME ${!NEON_TOKEN_ADDR_VAR} already exist"
  fi

  echo "$TOKEN_NAME successfully deployed"
}

DeployToken "Permission Allowance Token" NEON_PERMISSION_ALLOWANCE_TOKEN permission_allowance_token_keypair.json
DeployToken "Permission Denial Token" NEON_PERMISSION_DENIAL_TOKEN permission_denial_token_keypair.json

if [ "$SKIP_EVM_DEPLOY" != "YES" ]; then
    echo "Deploying evm_loader at address $EVM_LOADER..."
    if ! solana program deploy --upgrade-authority evm_loader-keypair.json evm_loader.so >evm_loader_id; then
        echo "Failed to deploy evm_loader"
        exit 1
    fi
    sleep 30
else
    echo "Skip deploying of evm_loader"
fi

DeployToken "Neon Token" NEON_TOKEN_MINT neon_token_keypair.json
export ETH_TOKEN_MINT=$NEON_TOKEN_MINT

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

echo "Creating EVM Loader token bank..."
python3 neon_pool_generator.py $EVM_LOADER $NEON_TOKEN_MINT
