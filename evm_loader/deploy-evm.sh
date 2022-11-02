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

if [ "$SKIP_EVM_DEPLOY" != "YES" ]; then
    echo "Deploying evm_loader at address $EVM_LOADER..."
    if ! solana program deploy --url $SOLANA_URL --upgrade-authority evm_loader-keypair.json \
        --program_id evm_loader-keypair.json evm_loader.so >/dev/null; then
      echo "Failed to deploy evm_loader"
      exit 1
    fi
    sleep 30
else
    echo "Skip deploying of evm_loader"
fi

neon-cli --url $SOLANA_URL --evm_loader $EVM_LOADER --loglevel warn \
  init-environment --send-trx --keys keys/