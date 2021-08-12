#!/usr/bin/env bash
# Build docker image to run the faucet service.
# The faucet binary should be built already.
set -e

rm -rf tmp
mkdir tmp
cp ../target/release/faucet tmp/
cp ../faucet.toml tmp/

docker build --force-rm --file Dockerfile --tag faucet .

rm -rf tmp
