#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

echo "REVISION=$REVISION"

set ${SOLANA_REVISION:=v1.8.12-testnet}

# Refreshing neonlabsorg/solana:latest image is required to run .buildkite/steps/build-image.sh locally
docker pull neonlabsorg/solana:${SOLANA_REVISION}
echo "SOLANA_REVISION=$SOLANA_REVISION"

docker build --build-arg REVISION=$REVISION --build-arg SOLANA_REVISION=$SOLANA_REVISION -t neonlabsorg/evm_loader:${REVISION} .
