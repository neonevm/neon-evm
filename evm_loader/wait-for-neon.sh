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

./wait-for-solana.sh "$@"

if [ -z "$1" ]; then
  if neon-cli --url $SOLANA_URL --evm_loader $EVM_LOADER --loglevel error init-environment; then
    exit 0
  fi
else
  WAIT_TIME=$1
  echo "Waiting $WAIT_TIME seconds for NeonEVM to be available at $SOLANA_URL:$EVM_LOADER"
  for i in $(seq 1 $WAIT_TIME); do
    if neon-cli --url $SOLANA_URL --evm_loader $EVM_LOADER --loglevel error init-environment; then
      exit 0
    fi
    sleep 1
  done
fi

echo "unable to connect to NeonEVM at $SOLANA_URL:$EVM_LOADER"
exit 1