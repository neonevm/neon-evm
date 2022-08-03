#!/usr/bin/env bash

set -e

SOLANA_BIN=/opt/solana/bin
NEON_BIN=/opt

DEPLOY_EVM_IN_GENESIS="${DEPLOY_IN_GENESIS:-YES}"

function deploy_tokens() {
    # deploy tokens needed by Neon EVM
    export SKIP_EVM_DEPLOY=$DEPLOY_EVM_IN_GENESIS
    export SOLANA_URL=http://127.0.0.1:8899

    cd ${NEON_BIN}
    ./wait-for-solana.sh 20
    ./deploy-evm.sh
}

deploy_tokens &

# run Solana with Neon EVM in genesis

cd ${SOLANA_BIN}

EVM_LOADER_SO=evm_loader.so
EVM_LOADER=$(${SOLANA_BIN}/solana address -k ${NEON_BIN}/evm_loader-keypair.json)
EVM_LOADER_PATH=${NEON_BIN}/${EVM_LOADER_SO}

cp ${EVM_LOADER_PATH} .

if [[ "$DEPLOY_EVM_IN_GENESIS" == "YES" ]]; then
  NEON_BPF_ARGS=(
      --bpf-program ${EVM_LOADER} BPFLoader2111111111111111111111111111111111 ${EVM_LOADER_SO}
  )
fi

NEON_VALIDATOR_ARGS=(
    --gossip-host $(hostname -i)
)

if [[ -n $GEYSER_PLUGIN_CONFIG ]]; then
  echo "Using geyser plugin with config: $GEYSER_PLUGIN_CONFIG"
  NEON_VALIDATOR_ARGS+=(--geyser-plugin-config $GEYSER_PLUGIN_CONFIG)
fi

export SOLANA_RUN_SH_GENESIS_ARGS="${NEON_BPF_ARGS[@]}"
export SOLANA_RUN_SH_VALIDATOR_ARGS="${NEON_VALIDATOR_ARGS[@]}"

./solana-run.sh
