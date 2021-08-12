#!/usr/bin/env bash
# Run docker container from the faucet service image.
set -e

docker run --name faucet --network host faucet
