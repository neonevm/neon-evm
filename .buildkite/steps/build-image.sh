#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

docker build --build-arg REVISION=${REVISION} -t cybercoredev/evm_loader:${REVISION} .
