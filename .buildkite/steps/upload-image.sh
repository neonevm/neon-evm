#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

docker images

docker login -u=${DHUBU} -p=${DHUBP}

docker push neonlabsorg/evm_loader:${REVISION}


