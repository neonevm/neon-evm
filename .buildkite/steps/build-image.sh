#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

echo "REVISION=$REVISION"

docker build --build-arg REVISION=$REVISION -t cybercoredev/evm_loader:${REVISION} .
