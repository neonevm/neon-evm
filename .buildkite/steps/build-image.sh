#!/bin/bash
set -euo pipefail

echo "Neon EVM revision=${BUILDKITE_COMMIT}"

set ${SOLANA_PROVIDER:=solanalabs}
set ${SOLANA_REVISION:=v1.11.10}

export SOLANA_IMAGE=${SOLANA_PROVIDER}/solana:${SOLANA_REVISION}
echo "SOLANA_IMAGE=${SOLANA_IMAGE}"
docker pull ${SOLANA_IMAGE}

docker build --build-arg REVISION=${BUILDKITE_COMMIT} --build-arg SOLANA_IMAGE=${SOLANA_IMAGE} -t neonlabsorg/evm_loader:${BUILDKITE_COMMIT} .
