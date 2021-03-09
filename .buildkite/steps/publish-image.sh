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

docker pull cybercoredev/evm_loader:${REVISION}
docker tag cybercoredev/evm_loader:${REVISION} cybercoredev/evm_loader:${TAG}
docker push cybercoredev/evm_loader:${TAG}

