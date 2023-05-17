#!/usr/bin/env bash

set -e

SOLANA_BIN=/opt/solana/bin
NEON_BIN=/opt

EVM_LOADER_SO=evm_loader.so
EVM_LOADER=$(${SOLANA_BIN}/solana address -k ${NEON_BIN}/evm_loader-keypair.json)
EVM_LOADER_PATH=${NEON_BIN}/${EVM_LOADER_SO}

function initialize_neon() {
    # deploy tokens needed by Neon EVM
    # temporary disable load NeonEVM in genesis
    #export SKIP_EVM_DEPLOY=${DEPLOY_EVM_IN_GENESIS:-NO}
    export SKIP_EVM_DEPLOY=NO
    export SOLANA_URL=http://127.0.0.1:8899
    export EVM_LOADER

    cd ${NEON_BIN}
    ./wait-for-solana.sh ${SOLANA_WAIT_TIMEOUT:-60}
    ./deploy-evm.sh
}

initialize_neon &

# run Solana with Neon EVM in genesis
cd ${SOLANA_BIN}
cp ${EVM_LOADER_PATH} .

# dump metaplex program from mainnet
METAPLEX=metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s
METAPLEX_SO=metaplex.so

if [[ "${DEPLOY_EVM_IN_GENESIS:-YES}" == "YES" ]]; then
# temporary disable load NeoneEVM in genesis
#  NEON_BPF_ARGS=(
#      --bpf-program ${EVM_LOADER} BPFLoader2111111111111111111111111111111111 ${EVM_LOADER_SO}
#      --bpf-program ${METAPLEX}   BPFLoader2111111111111111111111111111111111 ${METAPLEX_SO}
#  )
  NEON_BPF_ARGS=(
      --bpf-program ${METAPLEX}   BPFLoader2111111111111111111111111111111111 ${METAPLEX_SO}
  )
fi

NEON_VALIDATOR_ARGS=(
    --gossip-host $(hostname -i)
    --log-messages-bytes-limit 50000
)

if [[ -n $GEYSER_PLUGIN_CONFIG ]]; then
  echo "Using geyser plugin with config: $GEYSER_PLUGIN_CONFIG"
  NEON_VALIDATOR_ARGS+=(--geyser-plugin-config $GEYSER_PLUGIN_CONFIG)
fi

export SOLANA_RUN_SH_GENESIS_ARGS="${NEON_BPF_ARGS[@]}"
export SOLANA_RUN_SH_VALIDATOR_ARGS="${NEON_VALIDATOR_ARGS[@]}"

./solana-run.sh
