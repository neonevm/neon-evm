#!/usr/bin/env bash

if [ -z "${SOLANA_URL}" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

./wait-for-solana.sh "$@"

export EVM_LOADER=$(solana address -k evm_loader-keypair.json)
export $(neon-cli --evm_loader="${EVM_LOADER}" neon-elf-params ./evm_loader.so)
export ETH_TOKEN_MINT=$NEON_TOKEN_MINT

WAIT_TIME=${1:-1}
echo "Waiting ${WAIT_TIME} seconds for Neon EVM to be available at ${SOLANA_URL}"
for i in $(seq 1 ${WAIT_TIME}); do
    if python3 collateral_pool_checker.py collateral-pool-keypair.json > /dev/null 2>&1; then
        exit 0
    fi
    if [ ${i} -lt ${WAIT_TIME} ]; then
        sleep 1
    fi
done

echo "unable to connect to get the Neon EVM at ${SOLANA_URL}"
exit 1
