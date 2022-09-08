#!/usr/bin/env bash

set -e

SOLANA_BIN=/opt/solana/bin
NEON_BIN=/opt

function deploy_tokens() {
    # deploy tokens needed by Neon EVM
    export SKIP_EVM_DEPLOY=${DEPLOY_EVM_IN_GENESIS:-YES}
    export SOLANA_URL=http://127.0.0.1:8899

    cd ${NEON_BIN}
    ./wait-for-solana.sh ${SOLANA_WAIT_TIMEOUT:-60}
    ./deploy-evm.sh
}

deploy_tokens &

# run Solana with Neon EVM in genesis

cd ${SOLANA_BIN}

EVM_LOADER_SO=evm_loader.so
EVM_LOADER=$(${SOLANA_BIN}/solana address -k ${NEON_BIN}/evm_loader-keypair.json)
EVM_LOADER_PATH=${NEON_BIN}/${EVM_LOADER_SO}

cp ${EVM_LOADER_PATH} .

# dump metaplex program from mainnet
METAPLEX=metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s
METAPLEX_SO=metaplex.so

solana program dump ${METAPLEX} ${METAPLEX_SO} --url mainnet-beta

if [[ "${DEPLOY_EVM_IN_GENESIS:-YES}" == "YES" ]]; then
  NEON_BPF_ARGS=(
      --bpf-program ${EVM_LOADER} BPFLoader2111111111111111111111111111111111 ${EVM_LOADER_SO}
      --bpf-program ${METAPLEX}   BPFLoader2111111111111111111111111111111111 ${METAPLEX_SO}
  )
fi

NEON_VALIDATOR_ARGS=(
    --gossip-host $(hostname -i)
    --log-messages-bytes-limit 20000
)

if [[ -n $GEYSER_PLUGIN_CONFIG ]]; then
  echo "Using geyser plugin with config: $GEYSER_PLUGIN_CONFIG"
  NEON_VALIDATOR_ARGS+=(--geyser-plugin-config $GEYSER_PLUGIN_CONFIG)
fi

export SOLANA_RUN_SH_GENESIS_ARGS="${NEON_BPF_ARGS[@]}"
export SOLANA_RUN_SH_VALIDATOR_ARGS="${NEON_VALIDATOR_ARGS[@]}"

./solana-run.sh
