#!/bin/bash
set -euo pipefail

docker images

docker login -u=${DHUBU} -p=${DHUBP}

docker push neonlabsorg/evm_loader:${BUILDKITE_COMMIT}


