#!/bin/bash

cat << EOF
  - label: ":docker: build proxy docker image"
    trigger: "neon-proxy"
    build:
      branch: "${PROXY_BRANCH:-develop}"
      env:
          #TODO: to be dropped away
          EVM_LOADER_REVISION: "${BUILDKITE_COMMIT}"
          EVM_LOADER_BRANCH: "${EVM_LOADER_BRANCH}"
          #-----------------------
          SOLANA_REVISION: "v1.8.12-testnet"
          EVM_LOADER_FULL_TEST_SUITE: $(buildkite-agent meta-data get "full_test_suite" --default "false")
          NEON_EVM_COMMIT: "${BUILDKITE_COMMIT}"
          NEON_EVM_BRANCH: "${BUILDKITE_BRANCH}"
          NEON_EVM_REPO: "${BUILDKITE_REPO}"
EOF

