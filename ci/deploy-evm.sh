#!/bin/bash
set -euo pipefail

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

if [ -z "$EVM_LOADER" ]; then
  echo "EVM_LOADER is not set"
  exit 1
fi

echo "Deploying from " $(solana address) " with " $(solana balance)
if [ "$SKIP_EVM_DEPLOY" != "YES" ]; then
    echo "Deploying evm_loader at address $EVM_LOADER..."
    if ! solana program deploy --url $SOLANA_URL \
        --upgrade-authority evm_loader-keypair.json \
        --program-id evm_loader-keypair.json evm_loader.so; then
      echo "Failed to deploy evm_loader"
      exit 1
    fi
    echo "Deployed evm_loader at address $EVM_LOADER..."
    sleep 30
else
    echo "Skip deploying of evm_loader"
fi

echo "Deployed finished from " $(solana address) " with " $(solana balance)
neon-cli --url $SOLANA_URL --evm_loader $EVM_LOADER \
  --keypair evm_loader-keypair.json \
  --solana_key_for_config BMp6gEnveANdvSvspESJUrNczuHz1GF5UQKjVLCkAZih \
  --loglevel debug init-environment --send-trx --keys-dir keys/
