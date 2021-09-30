#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

echo "REVISION=$REVISION"

docker build --build-arg REVISION=$REVISION -t neonlabsorg/evm_loader:${REVISION} .
