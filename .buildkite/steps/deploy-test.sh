#!/bin/bash
set -euo pipefail

while getopts t: option; do
case "${option}" in
    t) IMAGETAG=${OPTARG};;
    *) echo "Usage: $0 [OPTIONS]. Where OPTIONS can be:"
       echo "    -t <IMAGETAG>  tag for neonlabsorg/evm_loader Docker-image"
       exit 1;;
esac
done

REVISION=$(git rev-parse HEAD)
EVM_LOADER_IMAGE=neonlabsorg/evm_loader:${IMAGETAG:-$REVISION}

echo "Currently runned Docker-containers"
docker ps -a

function cleanup_docker {
    docker logs solana >solana.log 2>&1
    echo "Cleanup docker-compose..."
    docker-compose -f evm_loader/docker-compose-test.yml down --timeout 60
    echo "Cleanup docker-compose done."
}
trap cleanup_docker EXIT

echo "\nCleanup docker-compose..."
docker-compose -f evm_loader/docker-compose-test.yml down

if ! docker-compose -f evm_loader/docker-compose-test.yml up -d; then
    echo "docker-compose failed to start"
    exit 1;
fi

# waiting for solana to launch
sleep 10

echo "Run tests..."
docker run --rm --network evm_loader-deploy_test-net -ti \
     -e SOLANA_URL=http://solana:8899 \
     ${EXTRA_ARGS:-} \
     $EVM_LOADER_IMAGE 'deploy-test.sh'
echo "Run tests return"

exit $?
