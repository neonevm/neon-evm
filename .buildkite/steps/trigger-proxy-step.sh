#!/bin/bash

cat << EOF
  - label: ":docker: build proxy docker image"
    trigger: "neon-proxy"
    build:
      branch: "772_test_gas_usage"
      env:
          NEON_EVM_FULL_TEST_SUITE: $(buildkite-agent meta-data get "full_test_suite" --default "false")
          NEON_EVM_COMMIT: "${BUILDKITE_COMMIT}"
          NEON_EVM_BRANCH: "${BUILDKITE_BRANCH}"
          NEON_EVM_REPO: "${BUILDKITE_REPO}"
EOF

