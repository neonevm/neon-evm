#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

echo "REVISION=$REVISION"

docker build --build-arg -t cybercoredev/evm_loader:${REVISION} .
