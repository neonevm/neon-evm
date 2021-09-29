#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

docker images

docker login -u=$DHUBU -p=$DHUBP

if [[ ${BUILDKITE_BRANCH} == "master" ]]; then
    TAG=stable
elif [[ ${BUILDKITE_BRANCH} == "develop" ]]; then
    TAG=latest
else
    TAG=${BUILDKITE_BRANCH}
fi

docker pull neonlabsorg/evm_loader:${REVISION}
docker tag neonlabsorg/evm_loader:${REVISION} neonlabsorg/evm_loader:${TAG}
docker push neonlabsorg/evm_loader:${TAG}

