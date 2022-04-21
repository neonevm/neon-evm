#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set -u "$SOLANA_URL"

solana airdrop 1

export TEST_PROGRAM=$(solana address -k proxy_program-keypair.json)

echo "Deploying proxy_program at address $TEST_PROGRAM..."
if ! solana program deploy --upgrade-authority proxy_program-keypair.json proxy_program.so >proxy_program; then
  echo "Failed to deploy proxy_program"
  exit 1
fi