#!/bin/bash

export TEST_PROGRAM=$(solana address -k test_program-keypair.json)

echo "Deploying test_program at address $TEST_PROGRAM..."
if ! solana program deploy --upgrade-authority test_program-keypair.json test_program.so >test_program; then
  echo "Failed to deploy evm_loader"
  exit 1
fi