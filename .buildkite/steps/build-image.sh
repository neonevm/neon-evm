#!/bin/bash
set -euo pipefail

echo "Neon EVM revision=${BUILDKITE_COMMIT}"

set ${SOLANA_REVISION:=v1.9.12-testnet}

docker pull neonlabsorg/solana:${SOLANA_REVISION}
echo "SOLANA_REVISION=$SOLANA_REVISION"

docker build --build-arg REVISION=${BUILDKITE_COMMIT} --build-arg SOLANA_REVISION=$SOLANA_REVISION -t neonlabsorg/evm_loader:${BUILDKITE_COMMIT} .
