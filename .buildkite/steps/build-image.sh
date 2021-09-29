#!/bin/bash
set -euo pipefail

REVISION=$(git rev-parse HEAD)

docker build -t neonlabsorg/evm_loader:${REVISION} .
